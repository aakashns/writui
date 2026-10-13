// Keep the text selectable when clipboard access is unavailable.
for (const button of document.querySelectorAll("button.copy")) {
  button.hidden = !navigator.clipboard;
  button.addEventListener("click", async () => {
    const text = document.getElementById(button.dataset.copy).textContent;
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = "Copied";
    } catch {
      button.textContent = "Couldn't copy";
    }
    setTimeout(() => { button.textContent = "Copy"; }, 1500);
  });
}
