// Torrz site: the theme switch and copy buttons. Everything works without it.
(function () {
  var root = document.documentElement;

  function systemDark() {
    return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches;
  }

  var toggle = document.querySelector(".theme-toggle");
  if (toggle) {
    toggle.addEventListener("click", function () {
      var current = root.getAttribute("data-theme") || (systemDark() ? "dark" : "light");
      var next = current === "dark" ? "light" : "dark";
      root.setAttribute("data-theme", next);
      try { localStorage.setItem("torrz-theme", next); } catch (e) {}
    });
  }

  // A copy button on every code block; the shell prompt and output aren't copied.
  document.querySelectorAll("pre").forEach(function (pre) {
    var button = document.createElement("button");
    button.type = "button";
    button.className = "copy small";
    button.dataset.variant = "secondary";
    button.textContent = "Copy";
    button.addEventListener("click", function () {
      var code = pre.querySelector("code") || pre;
      var text = code.getAttribute("data-copy");
      if (!text) {
        var clone = code.cloneNode(true);
        clone.querySelectorAll(".p, .o, .c, .ok").forEach(function (n) { n.remove(); });
        text = clone.textContent.replace(/\n{2,}/g, "\n").trim();
      }
      var done = function () {
        button.textContent = "Copied";
        setTimeout(function () { button.textContent = "Copy"; }, 1400);
      };
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text).then(done, function () {});
      }
    });
    pre.appendChild(button);
  });
})();
