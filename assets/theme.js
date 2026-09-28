(function () {
    var root = document.documentElement;
    var key = "fig-theme";
    try {
        if (localStorage.getItem(key) === "light") {
            root.dataset.figTheme = "light";
        }
    } catch (_) {
        // Storage can be unavailable (for example, in a restricted browser).
    }

    document.addEventListener("click", function (event) {
        if (!event.target.closest(".fig-theme-toggle")) return;
        var theme = root.dataset.figTheme === "light" ? "dark" : "light";
        if (theme === "light") {
            root.dataset.figTheme = "light";
        } else {
            delete root.dataset.figTheme;
        }
        try {
            localStorage.setItem(key, theme);
        } catch (_) {
            // The theme still changes for the current page.
        }
    });
})();
