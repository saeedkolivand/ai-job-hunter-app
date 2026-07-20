/* console easter egg — devtools hello, in the take-the-app voice */
  (function () { try {
    var head = 'color:#f4ecdc;background:#1c1812;font:700 18px/1.4 monospace;padding:6px 12px';
    var red = 'color:#e24b4a;font:13px/1.7 monospace';
    var soft = 'color:#6f6650;font:13px/1.7 monospace';
    console.log('%c ⬇ you opened the console, not the installer ', head);
    console.log('%crelatable. the buttons are right up there. source 👉 https://github.com/saeedkolivand/ai-job-hunter-app', red);
    console.log('%c(unsigned, local-first, free — the only cost is my dignity.)', soft);
  } catch (e) {} })();

  // Click (or Enter/Space) a .copy-cmd chip to copy its command to the clipboard,
  // with a brief ✓ confirmation. Commands carry the literal text in data-copy.
  document.querySelectorAll('.copy-cmd').forEach(function (el) {
    function copy() {
      var text = el.getAttribute('data-copy') || el.textContent || '';
      if (!navigator.clipboard) return;
      navigator.clipboard.writeText(text).then(function () {
        el.classList.add('copied');
        setTimeout(function () { el.classList.remove('copied'); }, 1300);
      });
    }
    el.addEventListener('click', copy);
    el.addEventListener('keydown', function (e) {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); copy(); }
    });
  });
