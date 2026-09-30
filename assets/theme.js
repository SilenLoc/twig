(function () {
    var root = document.documentElement;
    var key = "twig-theme";

    function applyTheme(theme) {
        if (theme === "light") {
            root.dataset.twigTheme = "light";
        } else {
            delete root.dataset.twigTheme;
        }

        document.querySelectorAll(".twig-theme-toggle").forEach(function (button) {
            var label = theme === "light" ? "Switch to dark mode" : "Switch to light mode";
            button.setAttribute("aria-label", label);
            button.setAttribute("title", label);
        });
    }

    try {
        applyTheme(localStorage.getItem(key) === "light" ? "light" : "dark");
    } catch (_) {
        // Storage can be unavailable (for example, in a restricted browser).
        applyTheme("dark");
    }

    document.addEventListener("click", function (event) {
        var target = event.target;
        if (!(target instanceof Element)) return;
        if (!target.closest(".twig-theme-toggle")) return;

        var theme = root.dataset.twigTheme === "light" ? "dark" : "light";
        applyTheme(theme);
        try {
            localStorage.setItem(key, theme);
        } catch (_) {
            // The theme still changes for the current page.
        }
    });
})();
