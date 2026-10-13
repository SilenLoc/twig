(function () {
  "use strict";

  var draft = null;
  var storageKey = null;
  var bound = false;
  var restoredKey = null;
  var initializedWorkspace = null;

  function workspace() {
    return document.getElementById("repo-edit-workspace");
  }

  function safeParse(value) {
    try {
      var parsed = JSON.parse(value);
      if (
        parsed &&
        typeof parsed.base_head === "string" &&
        Array.isArray(parsed.changes)
      )
        return parsed;
    } catch (_) {}
    return null;
  }

  function initialDraft(root) {
    var input = document.getElementById("repo-edit-draft-state");
    var initial = input ? safeParse(input.value) : null;
    return (
      initial || {
        base_head: root.dataset.baseHead || "",
        active_path: root.dataset.initialPath || null,
        tree_path: "",
        changes: [],
      }
    );
  }

  function loadDraft(root) {
    storageKey = root.dataset.draftKey;
    draft = initialDraft(root);
    var storedDraft = null;
    try {
      storedDraft = safeParse(window.sessionStorage.getItem(storageKey));
      if (storedDraft) draft = storedDraft;
    } catch (_) {}

    if (!Array.isArray(draft.changes)) draft.changes = [];
    if (typeof draft.tree_path !== "string") draft.tree_path = "";
    if (draft.base_head !== (root.dataset.baseHead || "")) {
      setStatus(
        "The repository revision changed. This tab's draft is preserved.",
        false,
      );
    }
    persistDraft();
    return Boolean(
      storedDraft &&
      (storedDraft.changes.length ||
        storedDraft.active_path ||
        storedDraft.tree_path),
    );
  }

  function persistDraft() {
    if (!draft) return;
        var serialized = JSON.stringify(draft);
        var input = document.getElementById("repo-edit-draft-state");
        if (input) input.value = serialized;
        var root = workspace();
        if (root) root.dataset.baseHead = draft.base_head;
    try {
      if (storageKey) window.sessionStorage.setItem(storageKey, serialized);
    } catch (_) {
      setStatus(
        "Browser storage is full. Copy or commit your draft before leaving this tab.",
        false,
      );
    }
    document
      .querySelectorAll(
        "#repo-edit-workspace form[data-repo-edit-operation] input[name=draft]",
      )
      .forEach(function (field) {
        field.value = serialized;
      });
  }

  function setStatus(message, success) {
    var target = document.getElementById("repo-edit-operation-result");
    if (!target) return;
    target.textContent = "";
    var paragraph = document.createElement("p");
    paragraph.className = success ? "twig-hint" : "twig-editor-status";
    paragraph.setAttribute("role", success ? "status" : "alert");
    paragraph.textContent = message;
    target.appendChild(paragraph);
  }

  function changeFor(path) {
    return (
      draft &&
      draft.changes.find(function (change) {
        return change.path === path;
      })
    );
  }

  function removeChange(path) {
    draft.changes = draft.changes.filter(function (change) {
      return change.path !== path;
    });
  }

  function upsertChange(path, content) {
    var change = changeFor(path);
    if (change) {
      change.state = { kind: "content", text: content };
    } else {
      draft.changes.push({
        path: path,
        state: { kind: "content", text: content },
      });
    }
  }

  function modeForPath(path) {
    return /\.(md|markdown)$/i.test(path);
  }

  function encodedPath(path) {
    return path.split("/").map(encodeURIComponent).join("/");
  }

  function fileUrl(path) {
    var root = workspace();
    return (
      "/" +
      encodeURIComponent(root.dataset.namespace) +
      "/" +
      encodeURIComponent(root.dataset.repo) +
      "/edit/file/" +
      encodedPath(path)
    );
  }

  function workspaceUrl(path) {
    var root = workspace();
    return (
      "/" +
      encodeURIComponent(root.dataset.namespace) +
      "/" +
      encodeURIComponent(root.dataset.repo) +
      "/edit?path=" +
      encodedPath(path)
    );
  }

  function operationUrl() {
    var root = workspace();
    return (
      "/" +
      encodeURIComponent(root.dataset.namespace) +
      "/" +
      encodeURIComponent(root.dataset.repo) +
      "/edit/operation"
    );
  }

  function syncWorkspaceAddress() {
    if (!draft || !window.history || !window.history.replaceState) return;
    var root = workspace();
    var path =
      "/" +
      encodeURIComponent(root.dataset.namespace) +
      "/" +
      encodeURIComponent(root.dataset.repo) +
      "/edit";
    var nextUrl = draft.active_path ? workspaceUrl(draft.active_path) : path;
    if (window.location.pathname + window.location.search !== nextUrl) {
      window.history.replaceState(window.history.state, "", nextUrl);
    }
  }

  function refreshTreeFromHead() {
    var root = workspace();
    var host = document.getElementById("repo-edit-tree");
    if (!root || !host || !window.htmx || !window.htmx.ajax) return;
    var url =
      "/" +
      encodeURIComponent(root.dataset.namespace) +
      "/" +
      encodeURIComponent(root.dataset.repo) +
      "/edit/tree/" +
      encodedPath(draft.tree_path || "");
    window.htmx.ajax("GET", url, { target: host, swap: "innerHTML" });
  }

  function refreshActiveFileFromHead() {
    var target = document.getElementById("repo-edit-document");
    if (!target) return;
    if (!draft.active_path) {
      loadActiveFile();
    } else if (window.htmx && window.htmx.ajax) {
      window.htmx.ajax("GET", fileUrl(draft.active_path), {
        target: target,
        swap: "innerHTML",
      });
    }
  }

  function markdownHtml(markdown) {
    return window.DOMPurify.sanitize(window.marked.parse(markdown));
  }

  function markdownFromRichText(quill) {
    var converter = new window.TurndownService({
      headingStyle: "atx",
      codeBlockStyle: "fenced",
      bulletListMarker: "-",
    });
    converter.addRule("fencedCodeBlock", {
      filter: function (node) {
        return (
          node.nodeName === "PRE" &&
          node.firstElementChild &&
          node.firstElementChild.nodeName === "CODE"
        );
      },
      replacement: function (_content, node) {
        var code = node.firstElementChild;
        var language =
          (code.className.match(/(?:^|\s)language-([\w-]+)/) || [])[1] || "";
        return (
          "\n\n```" +
          language +
          "\n" +
          code.textContent.replace(/\n+$/, "") +
          "\n```\n\n"
        );
      },
    });
    converter.addRule("quillCodeBlock", {
      filter: function (node) {
        return (
          node.nodeName === "DIV" &&
          node.classList.contains("ql-code-block-container")
        );
      },
      replacement: function (_content, node) {
        var blocks = Array.prototype.slice.call(
          node.querySelectorAll(".ql-code-block"),
        );
        var text = blocks
          .map(function (block) {
            return block.textContent;
          })
          .join("\n");
        return "\n\n```\n" + text.replace(/\n+$/, "") + "\n```\n\n";
      },
    });
    converter.addRule("quillList", {
      filter: function (node) {
        return (
          (node.nodeName === "OL" || node.nodeName === "UL") &&
          node.querySelector("li[data-list]")
        );
      },
      replacement: function (_content, node) {
        var items = Array.prototype.slice
          .call(node.children)
          .filter(function (child) {
            return child.nodeName === "LI";
          });
        var lines = items.map(function (item, index) {
          var content = item.cloneNode(true);
          content.querySelectorAll(".ql-ui").forEach(function (ui) {
            ui.remove();
          });
          content.removeAttribute("data-list");
          content.removeAttribute("class");
          var text = converter.turndown(content.innerHTML).trim();
          return (
            (item.dataset.list === "bullet" ? "- " : index + 1 + ". ") + text
          );
        });
        return "\n\n" + lines.join("\n") + "\n\n";
      },
    });
    converter.addRule("strikethrough", {
      filter: function (node) {
        return (
          node.nodeName === "S" ||
          node.nodeName === "STRIKE" ||
          node.nodeName === "DEL"
        );
      },
      replacement: function (content) {
        return "~~" + content + "~~";
      },
    });
    return converter.turndown(quill.root);
  }

  function currentContent(editor) {
    var source = editor.querySelector("[data-editor-source]");
    var quill = editor._repoEditQuill;
    if (
      editor.dataset.markdown === "true" &&
      editor.dataset.mode === "rich" &&
      quill
    ) {
      source.value = markdownFromRichText(quill);
    }
    return source.value;
  }

  function saveCurrentEditor(editor, markActive) {
    if (!draft || !editor) return;
    var path = editor.dataset.path;
    if (!path) return;
    var change = changeFor(path);
    if (change && change.state.kind === "deleted") return;
    var source = editor.querySelector("[data-editor-source]");
    var content = currentContent(editor);
    if (markActive) draft.active_path = path;
    if (
      editor.dataset.baseExists === "true" &&
      content === source.defaultValue
    ) {
      removeChange(path);
    } else {
      upsertChange(path, content);
    }
    persistDraft();
    renderWorkspaceState();
    var saveState = editor.querySelector("[data-repo-edit-save-state]");
    if (saveState) saveState.textContent = "Draft saved in this tab";
  }

  function setMode(editor, mode) {
    var source = editor.querySelector("[data-editor-source]");
    var rich = editor.querySelector("[data-editor-rich]");
    var toolbar = editor.querySelector("[data-editor-toolbar]");
    var buttons = editor.querySelectorAll("[data-editor-mode]");
    var isRich = mode === "rich";
    if (isRich) {
      var quill = editor._repoEditQuill;
      quill.setText("", "silent");
      quill.clipboard.dangerouslyPasteHTML(
        markdownHtml(source.value),
        "silent",
      );
    } else {
      source.value = markdownFromRichText(editor._repoEditQuill);
    }
    source.hidden = isRich;
    rich.hidden = !isRich;
    if (toolbar) toolbar.hidden = !isRich;
    editor.dataset.mode = mode;
    Array.prototype.forEach.call(buttons, function (button) {
      button.setAttribute(
        "aria-pressed",
        String(button.dataset.editorMode === mode),
      );
    });
    if (!isRich) saveCurrentEditor(editor, true);
  }

  function bindEditor(editor) {
    if (editor.dataset.repoEditBound) return;
    editor.dataset.repoEditBound = "true";
    var path = editor.dataset.path;
    var source = editor.querySelector("[data-editor-source]");
    var markdown = editor.dataset.markdown === "true";
    var change = changeFor(path);
    if (change && change.state.kind === "content")
      source.value = change.state.text;

    if (markdown) {
      var rich = editor.querySelector("[data-editor-rich]");
      var toolbar = editor.querySelector("[data-editor-toolbar]");
      var quill = new window.Quill(rich, {
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
      editor._repoEditQuill = quill;
      quill.clipboard.dangerouslyPasteHTML(
        markdownHtml(source.value),
        "silent",
      );
      quill.on("text-change", function (_delta, _old, sourceType) {
        if (sourceType === "user") saveCurrentEditor(editor, true);
      });
      editor.querySelectorAll("[data-editor-mode]").forEach(function (button) {
        button.addEventListener("click", function () {
          setMode(editor, button.dataset.editorMode);
        });
      });
      editor.dataset.mode = "rich";
      source.hidden = true;
      rich.hidden = false;
      if (toolbar) toolbar.hidden = false;
      editor
        .querySelectorAll('[data-editor-mode="rich"]')
        .forEach(function (button) {
          button.setAttribute("aria-pressed", "true");
        });
      editor
        .querySelectorAll('[data-editor-mode="source"]')
        .forEach(function (button) {
          button.setAttribute("aria-pressed", "false");
        });
    } else {
      source.addEventListener("input", function () {
        saveCurrentEditor(editor, true);
      });
    }
  }

  function hiddenField(form, name, value) {
    var field = form.querySelector('input[name="' + name + '"]');
    if (!field) {
      field = document.createElement("input");
      field.type = "hidden";
      field.name = name;
      form.appendChild(field);
    }
    field.value = value;
  }

  function addOperationForm(container, kind, fields) {
    var root = workspace();
    var form = document.createElement("form");
    form.className = "twig-repo-file-action-form";
    form.setAttribute("hx-post", operationUrl());
    form.setAttribute("hx-action", operationUrl());
    form.setAttribute("hx-method", "POST");
    form.setAttribute("hx-trigger", "submit");
        form.setAttribute("hx-target", "#repo-edit-operation-result");
        form.setAttribute("hx-swap", "innerHTML");
        form.setAttribute("data-repo-edit-operation", "");
        if (kind === "delete" && fields.path) {
            form.setAttribute("hx-confirm", "Delete " + fields.path + " in this pending workspace?");
        }
    hiddenField(form, "kind", kind);
    Object.keys(fields).forEach(function (key) {
      hiddenField(form, key, fields[key]);
    });
    hiddenField(form, "draft", JSON.stringify(draft));
    var button = document.createElement("button");
    button.type = "submit";
    button.className = "twig-btn twig-btn--quiet";
    button.textContent = kind === "discard" ? "Undo" : "Delete";
    form.appendChild(button);
    container.appendChild(form);
    if (window.htmx) window.htmx.process(form);
    return form;
  }

  function createEditor(path, content, baseExists) {
    var markdown = modeForPath(path);
    var editor = document.createElement("div");
    editor.className = "twig-repo-file-editor twig-editor";
    editor.dataset.path = path;
    editor.dataset.markdown = String(markdown);
    editor.dataset.baseExists = String(baseExists);

    var header = document.createElement("header");
    header.className = "twig-panel-head twig-repo-file-head";
    var pathCode = document.createElement("code");
    pathCode.className = "twig-mono";
    pathCode.textContent = path;
    var actions = document.createElement("div");
    actions.className = "twig-repo-file-actions";
    var renameDetails = document.createElement("details");
    var renameSummary = document.createElement("summary");
    renameSummary.className = "twig-btn twig-btn--quiet";
    renameSummary.textContent = "Rename";
    renameDetails.appendChild(renameSummary);
    var renameForm = document.createElement("form");
    renameForm.className = "twig-repo-file-action-form";
    renameForm.setAttribute("hx-post", operationUrl());
    renameForm.setAttribute("hx-action", operationUrl());
    renameForm.setAttribute("hx-method", "POST");
    renameForm.setAttribute("hx-trigger", "submit");
    renameForm.setAttribute("hx-target", "#repo-edit-operation-result");
    renameForm.setAttribute("hx-swap", "innerHTML");
    renameForm.setAttribute("data-repo-edit-operation", "");
    hiddenField(renameForm, "kind", "rename");
    hiddenField(renameForm, "from", path);
        var to = document.createElement("input");
        to.className = "twig-input";
        to.name = "to";
        to.setAttribute("aria-label", "New path");
    to.value = path;
    to.required = true;
    renameForm.appendChild(to);
    hiddenField(renameForm, "draft", JSON.stringify(draft));
    var renameButton = document.createElement("button");
    renameButton.type = "submit";
    renameButton.className = "twig-btn twig-btn--primary";
    renameButton.textContent = "Rename file";
    renameForm.appendChild(renameButton);
    renameDetails.appendChild(renameForm);
    actions.appendChild(renameDetails);
    addOperationForm(actions, "delete", { path: path });

    header.appendChild(pathCode);
    header.appendChild(actions);
    editor.appendChild(header);

    if (markdown) {
      var modeBar = document.createElement("nav");
      modeBar.className = "twig-editor-modes";
      modeBar.setAttribute("aria-label", "Editing mode");
      ["rich", "source"].forEach(function (mode) {
        var button = document.createElement("button");
        button.type = "button";
        button.className = "twig-btn twig-btn--quiet";
        button.dataset.editorMode = mode;
        button.setAttribute("aria-pressed", String(mode === "rich"));
        button.textContent = mode === "rich" ? "Rich text" : "Markdown source";
        modeBar.appendChild(button);
      });
      actions.appendChild(modeBar);

      var toolbar = document.createElement("div");
      toolbar.className = "twig-editor-toolbar ql-toolbar ql-snow";
      toolbar.dataset.editorToolbar = "";
      toolbar.innerHTML =
        '<span class="ql-formats"><button class="ql-bold" type="button" aria-label="Bold"></button><button class="ql-italic" type="button" aria-label="Italic"></button><button class="ql-strike" type="button" aria-label="Strikethrough"></button></span>' +
        '<span class="ql-formats"><select class="ql-header" aria-label="Heading"><option selected value="">Paragraph</option><option value="2">Heading 2</option><option value="3">Heading 3</option></select><button class="ql-blockquote" type="button" aria-label="Block quote"></button><button class="ql-code-block" type="button" aria-label="Code block"></button></span>' +
        '<span class="ql-formats"><button class="ql-list" type="button" value="ordered" aria-label="Numbered list"></button><button class="ql-list" type="button" value="bullet" aria-label="Bulleted list"></button><button class="ql-link" type="button" aria-label="Link"></button><button class="ql-clean" type="button" aria-label="Clear formatting"></button></span>';
      editor.appendChild(toolbar);
      var rich = document.createElement("div");
      rich.className = "twig-editor-rich";
      rich.dataset.editorRich = "";
      rich.setAttribute("aria-label", "Rich text editor for " + path);
      editor.appendChild(rich);
      var source = document.createElement("textarea");
      source.className = "twig-editor-source";
      source.dataset.editorSource = "";
      source.setAttribute("aria-label", "Markdown source for " + path);
      source.value = content;
      source.defaultValue = content;
      source.hidden = true;
      editor.appendChild(source);
    } else {
      var sourceText = document.createElement("textarea");
      sourceText.className = "twig-editor-source";
      sourceText.dataset.editorSource = "";
      sourceText.setAttribute("aria-label", "Source text for " + path);
      sourceText.value = content;
      sourceText.defaultValue = content;
      editor.appendChild(sourceText);
    }

    var footer = document.createElement("footer");
    footer.className = "twig-repo-file-foot";
    var type = document.createElement("span");
    type.className = "twig-row-meta";
    type.textContent = markdown ? "Markdown" : "UTF-8 text";
    var saved = document.createElement("span");
    saved.className = "twig-row-meta";
    saved.dataset.repoEditSaveState = "";
    saved.textContent = "Draft saved in this tab";
    footer.appendChild(type);
    footer.appendChild(saved);
    editor.appendChild(footer);
    return editor;
  }

  function renderDeletedFile(target, path) {
    target.textContent = "";
    var message = document.createElement("div");
    message.className = "twig-repo-file-unavailable";
    message.setAttribute("role", "status");
    var eyebrow = document.createElement("p");
    eyebrow.className = "twig-eyebrow";
    eyebrow.textContent = "MARKED FOR DELETION";
    var code = document.createElement("code");
    code.className = "twig-mono";
    code.textContent = path;
    var hint = document.createElement("p");
    hint.className = "twig-hint";
    hint.textContent =
      "This file will be removed when the workspace is committed.";
    var actions = document.createElement("div");
    addOperationForm(actions, "discard", { path: path });
    message.appendChild(eyebrow);
    message.appendChild(code);
    message.appendChild(hint);
    message.appendChild(actions);
    target.appendChild(message);
  }

  function renderEditor(target, path) {
    var change = changeFor(path);
    if (change && change.state.kind === "deleted") {
      renderDeletedFile(target, path);
      return;
    }
    if (
      change &&
      change.state.kind === "content" &&
      target.querySelector("[data-file-unavailable]")
    ) {
      target.textContent = "";
      target.appendChild(createEditor(path, change.state.text, false));
    }
    var editor = target.querySelector(".twig-repo-file-editor");
    if (!editor) return;
    var current = changeFor(path);
    var source = editor.querySelector("[data-editor-source]");
    if (source) {
      if (current && current.state.kind === "content") {
        source.value = current.state.text;
      } else if (editor.dataset.baseExists === "true") {
        source.value = source.defaultValue;
      }
      if (
        editor.dataset.repoEditBound &&
        editor._repoEditQuill &&
        editor.dataset.markdown === "true"
      ) {
        editor._repoEditQuill.setText("", "silent");
        editor._repoEditQuill.clipboard.dangerouslyPasteHTML(
          markdownHtml(source.value),
          "silent",
        );
      }
    }
    if (window.htmx) window.htmx.process(editor);
    bindEditor(editor);
  }

  function loadActiveFile() {
    if (!draft) return;
    var target = document.getElementById("repo-edit-document");
    if (!target) return;
    if (!draft.active_path) {
      target.textContent = "";
      var placeholder = document.createElement("div");
      placeholder.className = "twig-repo-edit-placeholder";
      var hint = document.createElement("p");
      hint.className = "twig-hint";
      hint.textContent = "Select a file to edit.";
      placeholder.appendChild(hint);
      target.appendChild(placeholder);
      return;
    }
    var rendered = target.querySelector("[data-path], [data-file-path]");
    var renderedPath =
      rendered && (rendered.dataset.path || rendered.dataset.filePath);
    if (rendered && renderedPath === draft.active_path) {
      renderEditor(target, draft.active_path);
      return;
    }
    if (window.htmx && window.htmx.ajax) {
      window.htmx.ajax("GET", fileUrl(draft.active_path), {
        target: target,
        swap: "innerHTML",
      });
    }
  }

  function renderChangeList() {
    var list = document.getElementById("repo-edit-change-list");
    var count = document.getElementById("repo-edit-change-count");
    var empty = document.getElementById("repo-edit-change-empty");
    if (!list || !count || !empty) return;
    list.textContent = "";
    count.textContent = String(draft.changes.length);
    empty.hidden = draft.changes.length > 0;
    draft.changes.forEach(function (change) {
      var item = document.createElement("li");
      var link = document.createElement("a");
      link.href = workspaceUrl(change.path);
      link.setAttribute("hx-get", fileUrl(change.path));
      link.setAttribute("hx-trigger", "click");
      link.setAttribute("hx-target", "#repo-edit-document");
      link.setAttribute("hx-swap", "innerHTML");
      link.setAttribute("hx-push-url", workspaceUrl(change.path));
      link.dataset.path = change.path;
      link.textContent = change.path;
      var marker = document.createElement("span");
      marker.className = "twig-row-meta";
      marker.textContent =
        change.state.kind === "deleted" ? "Deleted" : "Pending";
      item.appendChild(marker);
      item.appendChild(link);
      addOperationForm(item, "discard", { path: change.path });
      list.appendChild(item);
    });
    var commitButton = document.querySelector(
      "#repo-edit-commit-form button[type=submit]",
    );
    if (commitButton) {
      commitButton.disabled =
        draft.changes.length === 0 || workspace().dataset.hasEmail !== "true";
    }
    if (window.htmx) window.htmx.process(list);
  }

  function treeItem(path, directory, synthetic) {
    var item = document.createElement("li");
    item.setAttribute("data-repo-edit-tree-entry", "");
    item.dataset.path = path;
    item.dataset.directory = String(directory);
    item.dataset.baseExists = String(!synthetic);
    if (synthetic) item.dataset.repoEditSynthetic = "true";
    var anchor = document.createElement("a");
    anchor.className = "twig-repo-tree-link";
    anchor.dataset.path = path;
    anchor.dataset.directory = String(directory);
    anchor.dataset.baseExists = String(!synthetic);
    var namespace = workspace().dataset.namespace;
    var repo = workspace().dataset.repo;
    var icon = document.createElement("span");
    icon.className = "twig-repo-tree-icon";
    icon.setAttribute("aria-hidden", "true");
    icon.textContent = directory ? "▸" : "¶";
    anchor.appendChild(icon);
    anchor.appendChild(document.createTextNode(path.split("/").pop()));
    if (directory) {
      var url =
        "/" +
        encodeURIComponent(namespace) +
        "/" +
        encodeURIComponent(repo) +
        "/edit/tree/" +
        encodedPath(path);
      anchor.href = url;
      anchor.setAttribute("hx-get", url);
      anchor.setAttribute("hx-trigger", "click");
      anchor.setAttribute("hx-target", "#repo-edit-tree");
      anchor.setAttribute("hx-swap", "innerHTML");
    } else {
      anchor.href = workspaceUrl(path);
      anchor.setAttribute("hx-get", fileUrl(path));
      anchor.setAttribute("hx-trigger", "click");
      anchor.setAttribute("hx-target", "#repo-edit-document");
      anchor.setAttribute("hx-swap", "innerHTML");
      anchor.setAttribute("hx-push-url", workspaceUrl(path));
    }
    item.appendChild(anchor);
    return item;
  }

  function applyTreeDraft() {
    if (!draft) return;
    var host = document.getElementById("repo-edit-tree");
    var tree = host && host.querySelector("[data-repo-edit-tree-path]");
    var list = tree && tree.querySelector(".twig-repo-tree");
    if (!tree || !list) return;
    list
      .querySelectorAll("[data-repo-edit-synthetic]")
      .forEach(function (item) {
        item.remove();
      });
    var currentPath = tree.dataset.repoEditTreePath || "";
    draft.changes.forEach(function (change) {
      var relative = currentPath
        ? change.path.indexOf(currentPath + "/") === 0
          ? change.path.slice(currentPath.length + 1)
          : null
        : change.path;
      if (relative === null || relative === "") return;
      var segments = relative.split("/");
      var fullPath = currentPath
        ? currentPath + "/" + segments[0]
        : segments[0];
      var existing = Array.prototype.find.call(
        list.querySelectorAll("[data-repo-edit-tree-entry][data-path]"),
        function (item) {
          return item.dataset.path === fullPath;
        },
      );
      if (segments.length > 1) {
        if (!existing) list.appendChild(treeItem(fullPath, true, true));
        return;
      }
      if (existing) {
        var existingAnchor = existing.querySelector("a");
        if (existingAnchor) {
          existingAnchor.dataset.changeKind =
            change.state.kind === "deleted" ? "deleted" : "modified";
        }
      } else if (change.state.kind === "content") {
        var added = treeItem(fullPath, false, true);
        added.querySelector("a").dataset.changeKind = "added";
        list.appendChild(added);
      }
    });
    list
      .querySelectorAll("[data-repo-edit-tree-entry] a")
      .forEach(function (anchor) {
        var change = changeFor(anchor.dataset.path);
        if (!change) {
          delete anchor.dataset.changeKind;
        } else if (change.state.kind === "deleted") {
          anchor.dataset.changeKind = "deleted";
        } else {
          anchor.dataset.changeKind =
            anchor.dataset.baseExists === "false" ? "added" : "modified";
        }
      });
    var empty = list.querySelector(".twig-repo-tree-empty");
    if (empty && list.querySelectorAll("[data-repo-edit-tree-entry]").length)
      empty.remove();
    if (window.htmx) window.htmx.process(list);
  }

  function loadTreePathIfNeeded() {
    if (!draft || !window.htmx || !window.htmx.ajax) return;
    var host = document.getElementById("repo-edit-tree");
    var tree = host && host.querySelector("[data-repo-edit-tree-path]");
    if (!tree || tree.dataset.repoEditTreePath === draft.tree_path) return;
    var root = workspace();
    var url =
      "/" +
      encodeURIComponent(root.dataset.namespace) +
      "/" +
      encodeURIComponent(root.dataset.repo) +
      "/edit/tree/" +
      encodedPath(draft.tree_path);
    window.htmx.ajax("GET", url, { target: host, swap: "innerHTML" });
  }

  function renderWorkspaceState() {
    persistDraft();
    renderChangeList();
    applyTreeDraft();
  }

  function responseAfterSwap(event) {
    var context = event.detail && (event.detail.ctx || event.detail);
    var target = context && context.target;
    if (!target) return;
    if (target.id === "repo-edit-commit-result") {
      var commitResult = target.firstElementChild;
      if (commitResult && commitResult.dataset.draft) {
        var committedDraft = safeParse(commitResult.dataset.draft);
        if (committedDraft) draft = committedDraft;
      }
      persistDraft();
      renderWorkspaceState();
      if (commitResult && commitResult.dataset.commitSuccess === "true") {
        var commitForm = document.getElementById("repo-edit-commit-form");
        var message = commitForm && commitForm.elements.namedItem("message");
        if (message) message.value = "";
        var changesPanel = document.getElementById("repo-edit-changes-panel");
        if (changesPanel) changesPanel.open = true;
        setStatus(commitResult.textContent, true);
        syncWorkspaceAddress();
        refreshTreeFromHead();
        refreshActiveFileFromHead();
      }
      return;
    }
    if (target.id === "repo-edit-operation-result") {
      var result = target.firstElementChild;
      if (result && result.dataset.draft) {
        var returned = safeParse(result.dataset.draft);
        if (returned) draft = returned;
      }
      persistDraft();
      renderWorkspaceState();
      if (result && result.dataset.operationSuccess === "true") {
        setStatus(result.textContent, true);
        syncWorkspaceAddress();
        loadTreePathIfNeeded();
        loadActiveFile();
      }
      return;
    }
    if (target.id === "repo-edit-document") {
      var unavailable = target.querySelector("[data-file-unavailable]");
      var path = unavailable
        ? unavailable.dataset.filePath
        : (target.querySelector(".twig-repo-file-editor") || {}).dataset?.path;
      if (path) renderEditor(target, path);
    }
    if (target.id === "repo-edit-tree") {
      applyTreeDraft();
      loadTreePathIfNeeded();
    }
  }

  function beforeRequest(event) {
    if (!draft) return;
    var context = event.detail && (event.detail.ctx || event.detail);
    var element = context && (context.sourceElement || context.elt);
    if (!element) element = event.target;
    if (!element) return;
    var operationForm =
      element.closest && element.closest("form[data-repo-edit-operation]");
    var commitForm =
      element.closest && element.closest("form[data-repo-edit-commit]");
    var pathElement = element.closest && element.closest("[data-path]");
    var isUserNavigation =
      pathElement &&
      (pathElement.dataset.directory === "false" ||
        pathElement.dataset.directory === "true");
    if (operationForm || commitForm || isUserNavigation) {
      var editor = document.querySelector(
        "#repo-edit-document .twig-repo-file-editor",
      );
      if (editor) saveCurrentEditor(editor, false);
    }
    if (pathElement && pathElement.dataset.directory === "false") {
      draft.active_path = pathElement.dataset.path;
    } else if (pathElement && pathElement.dataset.directory === "true") {
      draft.tree_path = pathElement.dataset.path;
    }
    if (operationForm || commitForm) {
      var formData = context && context.request && context.request.body;
      if (formData && typeof formData.set === "function") {
        formData.set("draft", JSON.stringify(draft));
      }
    }
    persistDraft();
  }

  function restoreWorkspace() {
    var root = workspace();
    if (!root || root === initializedWorkspace) return;
    if (!bound) {
      bound = true;
      document.addEventListener("htmx:config:request", beforeRequest);
      document.addEventListener("htmx:after:swap", responseAfterSwap);
      document.addEventListener("input", function (event) {
        if (
          event.target.matches &&
          event.target.matches("[data-editor-source]")
        ) {
          var editor = event.target.closest(".twig-repo-file-editor");
          if (editor) saveCurrentEditor(editor, false);
        }
      });
    }

    var oldKey = storageKey;
    var shouldRestore = false;
    if (oldKey !== root.dataset.draftKey || !draft)
      shouldRestore = loadDraft(root);
    initializedWorkspace = root;
    var hidden = document.getElementById("repo-edit-draft-state");
    var initialPath = root.dataset.initialPath || null;
    if (draft.active_path && draft.active_path !== initialPath && window.htmx) {
      window.htmx.ajax("GET", fileUrl(draft.active_path), {
        target: document.getElementById("repo-edit-document"),
        swap: "innerHTML",
      });
    }
    if (hidden && draft.active_path) hidden.value = JSON.stringify(draft);
    renderWorkspaceState();
    if (
      shouldRestore &&
      restoredKey !== storageKey &&
      window.htmx &&
      window.htmx.trigger
    ) {
      var restoreForm = document.getElementById("repo-edit-restore-form");
      if (restoreForm) {
        restoredKey = storageKey;
        window.htmx.process(restoreForm);
        window.htmx.trigger(restoreForm, "repo-edit-restore");
      }
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", restoreWorkspace);
  } else {
    restoreWorkspace();
  }
  document.addEventListener("htmx:after:swap", function (event) {
    var context = event.detail && (event.detail.ctx || event.detail);
    if (
      context &&
      context.target &&
      context.target.id === "repo-edit-workspace"
    ) {
      restoreWorkspace();
    }
  });
})();
