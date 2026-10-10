(function () {
    "use strict";

    var form = document.getElementById("repository-editor-form");
    if (!form || form.dataset.bound) return;
    form.dataset.bound = "true";

    var source = form.querySelector("#repository-editor-content");
    var status = form.querySelector("[data-editor-status]");
    var submit = form.querySelector('[type="submit"]');
    var reload = form.querySelector("[data-editor-reload]");
    var copyButton = form.querySelector("[data-editor-conflict-copy]");
    var isMarkdown = form.dataset.markdown === "true";
    var quill = null;
    var richChanged = false;

    function markdownHtml(markdown) {
        return window.DOMPurify.sanitize(window.marked.parse(markdown));
    }

    function markdownFromRichText() {
        var converter = new window.TurndownService({
            headingStyle: "atx",
            codeBlockStyle: "fenced",
            bulletListMarker: "-",
        });
        converter.addRule("fencedCodeBlock", {
            filter: function (node) {
                return node.nodeName === "PRE" && node.firstElementChild && node.firstElementChild.nodeName === "CODE";
            },
            replacement: function (_content, node) {
                var code = node.firstElementChild;
                var language = (code.className.match(/(?:^|\s)language-([\w-]+)/) || [])[1] || "";
                var text = code.textContent.replace(/\n+$/, "");
                return "\n\n```" + language + "\n" + text + "\n```\n\n";
            },
        });
        converter.addRule("quillCodeBlock", {
            filter: function (node) {
                return node.nodeName === "DIV" && node.classList.contains("ql-code-block-container");
            },
            replacement: function (_content, node) {
                var blocks = Array.prototype.slice.call(node.querySelectorAll(".ql-code-block"));
                var text = blocks.map(function (block) { return block.textContent; }).join("\n");
                return "\n\n```\n" + text.replace(/\n+$/, "") + "\n```\n\n";
            },
        });
        converter.addRule("quillList", {
            filter: function (node) {
                return (node.nodeName === "OL" || node.nodeName === "UL") && node.querySelector("li[data-list]");
            },
            replacement: function (_content, node) {
                var items = Array.prototype.slice.call(node.children).filter(function (child) {
                    return child.nodeName === "LI";
                });
                var lines = items.map(function (item, index) {
                    var content = item.cloneNode(true);
                    content.querySelectorAll(".ql-ui").forEach(function (ui) { ui.remove(); });
                    content.removeAttribute("data-list");
                    content.removeAttribute("class");
                    var text = converter.turndown(content.innerHTML).trim();
                    var marker = item.dataset.list === "bullet" ? "- " : (index + 1) + ". ";
                    return marker + text;
                });
                return "\n\n" + lines.join("\n") + "\n\n";
            },
        });
        converter.addRule("strikethrough", {
            filter: function (node) {
                return node.nodeName === "S" || node.nodeName === "STRIKE" || node.nodeName === "DEL";
            },
            replacement: function (content) {
                return "~~" + content + "~~";
            },
        });
        return converter.turndown(quill.root);
    }

    if (isMarkdown) {
        var rich = form.querySelector("[data-editor-rich]");
        var toolbar = form.querySelector("[data-editor-toolbar]");
        var modes = Array.prototype.slice.call(form.querySelectorAll("[data-editor-mode]"));
        quill = new window.Quill(rich, {
            theme: "snow",
            modules: { toolbar: toolbar, history: { delay: 1000, maxStack: 100, userOnly: true } },
            formats: ["bold", "italic", "strike", "link", "header", "list", "blockquote", "code", "code-block"],
        });
        quill.root.setAttribute("role", "textbox");
        quill.root.setAttribute("aria-multiline", "true");
        quill.root.setAttribute("aria-label", rich.getAttribute("aria-label"));
        quill.on("text-change", function (_delta, _old, sourceType) {
            if (sourceType === "user") richChanged = true;
        });

        function setMode(mode, focusEditor) {
            var isRich = mode === "rich";
            if (isRich && !richChanged) {
                quill.setText("", "silent");
                quill.clipboard.dangerouslyPasteHTML(markdownHtml(source.value), "silent");
            } else if (!isRich && richChanged) {
                source.value = markdownFromRichText();
                richChanged = false;
            }
            source.hidden = isRich;
            rich.hidden = !isRich;
            toolbar.hidden = !isRich;
            modes.forEach(function (button) {
                button.setAttribute("aria-pressed", String(button.dataset.editorMode === mode));
            });
            if (isRich && focusEditor) quill.focus();
        }

        modes.forEach(function (button) {
            button.addEventListener("click", function () {
                setMode(button.dataset.editorMode, true);
            });
        });
        setMode("rich", false);
    }

    async function save(saveAsConflictCopy) {
        if (isMarkdown && richChanged) {
            source.value = markdownFromRichText();
            richChanged = false;
        }
        submit.disabled = true;
        copyButton.disabled = true;
        status.textContent = saveAsConflictCopy ? "Saving draft as a new file…" : "Committing…";
        try {
            var response = await fetch(form.dataset.saveUrl, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    content: source.value,
                    message: form.querySelector("#repository-editor-message").value,
                    expected_head: form.querySelector('[name="expected_head"]').value,
                    save_conflict_copy: saveAsConflictCopy,
                }),
            });
            if (!response.ok) {
                var error = new Error((await response.text()) || "The change could not be committed.");
                error.status = response.status;
                throw error;
            }
            var result = await response.json();
            window.location.assign(result.location);
        } catch (error) {
            status.textContent = error.message || "The change could not be committed.";
            if (error.status === 409 && !saveAsConflictCopy) {
                reload.hidden = false;
                copyButton.hidden = false;
                copyButton.disabled = false;
            } else if (saveAsConflictCopy) {
                copyButton.disabled = false;
            } else {
                submit.disabled = false;
            }
        }
    }

    form.addEventListener("submit", function (event) {
        event.preventDefault();
        save(false);
    });
    copyButton.addEventListener("click", function () {
        save(true);
    });
})();
