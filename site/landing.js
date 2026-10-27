// The inline bootstrap prepares entrances before paint. This deferred script
// starts them and connects the controls once the document is ready.
(function () {
  var capable = window.matchMedia && Element.prototype.animate && 'IntersectionObserver' in window;
  var reduced = capable && window.matchMedia('(prefers-reduced-motion: reduce)');

  document.addEventListener('DOMContentLoaded', function () {

    // Copy the generation command. Falls back to selecting the text.
    document.querySelectorAll('button[data-copy]').forEach(function (button) {
      button.addEventListener('click', function () {
        var command = document.getElementById(button.dataset.copy);
        var status = document.getElementById('copy-status');
        function selectCommand() {
          var range = document.createRange();
          range.selectNodeContents(command);
          var selection = window.getSelection();
          selection.removeAllRanges();
          selection.addRange(range);
          status.textContent = 'Command selected. Press Ctrl+C or use your device’s copy action.';
        }
        if (!navigator.clipboard || !window.isSecureContext) { selectCommand(); return; }
        navigator.clipboard.writeText(command.textContent).then(function () {
          button.textContent = 'Copied';
          status.textContent = 'Copied to the clipboard';
          setTimeout(function () { button.textContent = 'Copy command'; }, 1600);
        }, selectCommand);
      });
    });

    if (!capable) return;

    // Entrances: the hero words rise once, then each block fades up as it enters view.
    var entrances = new Map();
    var revealTargets = Array.from(document.querySelectorAll('[data-reveal]'));
    var words = Array.from(document.querySelectorAll('[data-word]'));
    function finishEntrance(element) {
      element.setAttribute('data-revealed', '');
      var animation = entrances.get(element);
      if (animation) animation.finish();
    }
    if (reduced.matches) {
      words.concat(revealTargets).forEach(function (element) { element.setAttribute('data-revealed', ''); });
    } else {
      words.forEach(function (word, index) {
        var animation = word.animate(
          [{ transform: 'translateY(100%)' }, { transform: 'translateY(0)' }],
          { duration: 700, delay: 120 + index * 90, easing: 'cubic-bezier(.2,.7,.2,1)', fill: 'backwards' });
        entrances.set(word, animation);
        animation.onfinish = function () { entrances.delete(word); };
        word.setAttribute('data-revealed', '');
      });
      var entranceObserver = new IntersectionObserver(function (entries) {
        var delay = 0;
        entries.forEach(function (entry) {
          if (!entry.isIntersecting) return;
          var element = entry.target;
          entranceObserver.unobserve(element);
          var animation = element.animate(
            [{ opacity: 0, transform: 'translateY(32px)' }, { opacity: 1, transform: 'translateY(0)' }],
            { duration: 1000, delay: delay, easing: 'cubic-bezier(.25,.1,.25,1)', fill: 'backwards' });
          entrances.set(element, animation);
          animation.onfinish = function () { entrances.delete(element); };
          delay = Math.min(delay + 120, 360);
          element.setAttribute('data-revealed', '');
        });
      }, { rootMargin: '0px 0px -48px 0px', threshold: .05 });
      revealTargets.forEach(function (element) { entranceObserver.observe(element); });
      document.addEventListener('focusin', function (event) {
        revealTargets.forEach(function (element) {
          if (!element.hasAttribute('data-revealed') && element.contains(event.target)) {
            entranceObserver.unobserve(element);
            finishEntrance(element);
          }
        });
      });
    }

    // Decorative motion: ambient glows, the orbit dot, the aura and the thermostat
    // illustration. One page-wide control pauses all of it. Loops run only in view,
    // and the HTML already shows the complete final result.
    (function () {
      var button = document.getElementById('motion-toggle');
      var status = document.getElementById('demo-state');
      var figure = document.getElementById('model-illustration');
      if (!button) return;
      var requested = true;
      var effects = new Map();
      function pause(animation) {
        var time = animation.currentTime;
        animation.pause();
        if (time !== null) animation.currentTime = time;
      }

      document.querySelectorAll('[data-motion], [data-aura]').forEach(function (element) {
        var kind = element.dataset.motion;
        var frames;
        var duration;
        if (kind === 'orbit') {
          frames = [{ transform: 'rotate(0deg)' }, { transform: 'rotate(360deg)' }];
          duration = 40000;
        } else if (kind === 'breathe') {
          // Match the hero glow's opacity-25 base level without reading styles.
          frames = [{ opacity: .15, transform: 'scale(.96)' },
            { opacity: .25, transform: 'scale(1.05)' },
            { opacity: .15, transform: 'scale(.96)' }];
          duration = 7000;
        }
        var animation = frames && element.animate(frames, {
          duration: duration, iterations: Infinity, easing: kind === 'breathe' ? 'ease-in-out' : 'linear'
        });
        if (animation) animation.pause();
        effects.set(element, { animation: animation, visible: false });
      });

      // The illustration: four values arrive on the range, one at a time.
      var LOOP = 14000;
      function appear(at) {
        return [
          { offset: 0, opacity: 0, transform: 'translateY(-10px)' },
          { offset: at, opacity: 0, transform: 'translateY(-10px)' },
          { offset: Math.min(at + .03, .9), opacity: 1, transform: 'translateY(0)' },
          { offset: .92, opacity: 1, transform: 'translateY(0)' },
          { offset: .97, opacity: 0, transform: 'translateY(0)' },
          { offset: 1, opacity: 0, transform: 'translateY(0)' }
        ];
      }
      var sceneFrames = {
        band: [
          { offset: 0, transform: 'scaleX(0)' }, { offset: .02, transform: 'scaleX(0)' },
          { offset: .1, transform: 'scaleX(1)' }, { offset: .92, transform: 'scaleX(1)' },
          { offset: .97, transform: 'scaleX(1)', opacity: 1 }, { offset: 1, transform: 'scaleX(1)', opacity: 0 }
        ],
        v5: appear(.16), v30: appear(.3), v31: appear(.44), loupe: appear(.58), v525: appear(.66)
      };
      var scenes = figure ? Array.from(figure.querySelectorAll('[data-scene]')).map(function (scene) {
        var animation = scene.animate(sceneFrames[scene.dataset.scene], { duration: LOOP, iterations: Infinity, easing: 'linear' });
        animation.pause();
        return animation;
      }) : [];
      var figureVisible = false;
      var hovered = false;
      var focused = false;

      function update() {
        var paused = !requested || reduced.matches;
        document.documentElement.toggleAttribute('data-motion-paused', paused);
        effects.forEach(function (effect, element) {
          var running = !paused && !document.hidden && effect.visible;
          element.toggleAttribute('data-motion-active', running);
          if (!effect.animation) return;
          if (reduced.matches) effect.animation.cancel();
          else if (running) effect.animation.play();
          else pause(effect.animation);
        });
        var demoRunning = !paused && !document.hidden && figureVisible && !hovered && !focused;
        scenes.forEach(function (animation) {
          if (reduced.matches) animation.cancel();
          else if (demoRunning) animation.play();
          else pause(animation);
        });
        if (status) {
          status.textContent = reduced.matches ? 'Reduced motion: final result' : !requested ? 'Paused' :
            document.hidden ? 'Paused while the page is hidden' : !figureVisible ? 'Paused outside view' :
            hovered ? 'Paused on hover' : focused ? 'Paused on focus' : 'Playing · 14-second loop';
        }
        button.textContent = requested ? 'Pause motion' : 'Resume motion';
        button.disabled = reduced.matches;
        if (reduced.matches) button.textContent = 'Motion disabled';
      }

      var observer = new IntersectionObserver(function (entries) {
        entries.forEach(function (entry) { effects.get(entry.target).visible = entry.isIntersecting; });
        update();
      }, { threshold: .1 });
      effects.forEach(function (_, element) { observer.observe(element); });
      if (figure) {
        figure.addEventListener('mouseenter', function () { hovered = true; update(); });
        figure.addEventListener('mouseleave', function () { hovered = false; update(); });
        figure.addEventListener('focusin', function () { focused = true; update(); });
        figure.addEventListener('focusout', function () { focused = false; update(); });
        new IntersectionObserver(function (entries) {
          figureVisible = entries[0].isIntersecting && entries[0].intersectionRatio >= .2;
          update();
        }, { threshold: .2 }).observe(figure);
      }
      button.addEventListener('click', function () { requested = !requested; update(); });
      reduced.addEventListener('change', update);
      document.addEventListener('visibilitychange', update);
      button.hidden = false;
      update();
    })();
  });
})();
