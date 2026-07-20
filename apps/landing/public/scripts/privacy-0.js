/* console easter egg — devtools hello, in the privacy voice */
(function(){try{
  var head='color:#f4ecdc;background:#1c1812;font:700 18px/1.4 monospace;padding:6px 12px';
  var red='color:#e24b4a;font:13px/1.7 monospace';
  var soft='color:#6f6650;font:13px/1.7 monospace';
  console.log('%c 🔒 still not tracking you ', head);
  console.log('%cnot even in here. no analytics opened this console. source 👉 https://github.com/saeedkolivand/ai-job-hunter-app', red);
  console.log('%c(we can barely track ourselves. the policy above matches the actual code.)', soft);
}catch(e){}})();
