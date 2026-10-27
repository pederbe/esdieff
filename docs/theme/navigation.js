// Retain mdBook's sidebar persistence and resizing through its native checkbox.
(function () {
  // mdBook renders the sidebar toggle as a label, which may not carry ARIA state.
  // A button with the same name and state keeps the native checkbox in charge.
  var label = document.getElementById("mdbook-sidebar-toggle");
  if (label && label.tagName === "LABEL") {
    var replacement = document.createElement("button");
    replacement.type = "button";
    replacement.id = label.id;
    replacement.className = label.className;
    ["aria-label", "aria-controls", "aria-expanded", "title"].forEach(function (name) {
      if (label.hasAttribute(name)) replacement.setAttribute(name, label.getAttribute(name));
    });
    replacement.innerHTML = label.innerHTML;
    label.replaceWith(replacement);
  }
  var button = document.getElementById("mdbook-sidebar-toggle");
  var checkbox = document.getElementById("mdbook-sidebar-toggle-anchor");
  if (button && checkbox) {
    button.addEventListener("click", function () { checkbox.click(); });
    checkbox.addEventListener("change", function () { button.setAttribute("aria-expanded", String(checkbox.checked)); });
  }
  document.addEventListener("DOMContentLoaded", function () {
    // Start chapter submenus collapsed, including the current chapter's ancestors.
    document.querySelectorAll(".chapter-fold-toggle").forEach(function (toggle) {
      var item = toggle.closest("li");
      var chapter = toggle.previousElementSibling;
      item.classList.remove("expanded");
      toggle.setAttribute("role", "button");
      toggle.setAttribute("aria-label", "Toggle sections under " + chapter.textContent.trim());
      function updateExpanded() {
        toggle.setAttribute("aria-expanded", item.classList.contains("expanded"));
      }
      updateExpanded();
      new MutationObserver(updateExpanded).observe(item, { attributes: true, attributeFilter: ["class"] });
      toggle.addEventListener("keydown", function (event) {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          toggle.click();
        }
      });
    });
    function updateTabStops() {
      var visible = document.documentElement.classList.contains("sidebar-visible");
      document.querySelectorAll("#mdbook-sidebar a").forEach(function (link) {
        link.tabIndex = visible ? 0 : -1;
      });
    }
    updateTabStops();
    new MutationObserver(updateTabStops).observe(document.documentElement, {
      attributes: true, attributeFilter: ["class"]
    });
  });
})();
