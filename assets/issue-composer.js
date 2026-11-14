// Markdown fields for issues. The textarea holds the Markdown and is what gets posted. The
// Rich text mode edits it through Quill: switching to it renders the Markdown, and each
// edit writes the Markdown back. Without these libraries the fields stay plain textareas.
(function () {
  "use strict";

  function markdownHtml(markdown) {
    return window.DOMPurify.sanitize(window.marked.parse(markdown));
  }

  function markdownFromRichText(quill) {
    var converter = new window.TurndownService({
      headingStyle: "atx",
      codeBlockStyle: "fenced",
      bulletListMarker: "-",
    });
    // Quill marks code blocks with <pre> alone, so keep their text as a fenced block.
    converter.addRule("quillCodeBlock", {
      filter: "pre",
      replacement: function (_content, node) {
        var text = node.textContent.replace(/\n$/, "");
        return "\n\n```\n" + text + "\n```\n\n";
      },
    });
    converter.addRule("strikethrough", {
      filter: ["s", "del", "strike"],
      replacement: function (content) {
        return "~~" + content + "~~";
      },
    });
    return converter.turndown(quill.root);
  }

  function enhance(composer) {
    var source = composer.querySelector("[data-editor-source]");
    var rich = composer.querySelector("[data-editor-rich]");
    var toolbar = composer.querySelector("[data-editor-toolbar]");
    var modes = composer.querySelector("[data-editor-modes]");
    var buttons = composer.querySelectorAll("[data-editor-mode]");
    var quill = null;

    function setMode(mode) {
      var isRich = mode === "rich";
      if (isRich) {
        if (!quill) createQuill();
        quill.setText("", "silent");
        quill.clipboard.dangerouslyPasteHTML(markdownHtml(source.value), "silent");
      }
      source.hidden = isRich;
      rich.hidden = !isRich;
      toolbar.hidden = !isRich;
      Array.prototype.forEach.call(buttons, function (button) {
        button.setAttribute("aria-pressed", String(button.dataset.editorMode === mode));
      });
    }

    function createQuill() {
      quill = new window.Quill(rich, {
        theme: "snow",
        modules: {
          toolbar: toolbar,
          history: { delay: 1000, maxStack: 100, userOnly: true },
        },
        formats: [
          "bold",
          "italic",
          "strike",
          "link",
          "header",
          "list",
          "blockquote",
          "code",
          "code-block",
        ],
      });
      quill.root.setAttribute("role", "textbox");
      quill.root.setAttribute("aria-multiline", "true");
      quill.root.setAttribute("aria-label", rich.getAttribute("aria-label"));
      quill.on("text-change", function (_delta, _old, sourceType) {
        if (sourceType === "user") source.value = markdownFromRichText(quill);
      });
    }

    Array.prototype.forEach.call(buttons, function (button) {
      button.addEventListener("click", function () {
        setMode(button.dataset.editorMode);
      });
    });
    modes.hidden = false;
  }

  if (!window.Quill || !window.marked || !window.DOMPurify || !window.TurndownService) {
    return;
  }
  document.querySelectorAll("[data-md-composer]").forEach(enhance);
})();
