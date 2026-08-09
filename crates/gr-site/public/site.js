const handoff = document.querySelector(".handoff");
const progressPath = document.querySelector(".handoff__progress");
const signal = document.querySelector(".handoff__signal");
const check = document.querySelector(".handoff__check");
const midpoint = document.querySelector(".handoff__label--mid");
const endpoint = document.querySelector(".handoff__label--end");
const copyButton = document.querySelector(".copy-button");
const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

const fallbackMotion = {
  ease_out_cubic(progress) {
    const value = Math.min(1, Math.max(0, progress));
    return 1 - (1 - value) ** 3;
  },
  signal_radius(progress) {
    const value = Math.min(1, Math.max(0, progress));
    return 3.2 + 1.6 * Math.exp(-(((value - 0.5) / 0.1) ** 2));
  },
  midpoint_opacity(progress) {
    const value = Math.min(1, Math.max(0, progress));
    const fadeIn = Math.min(1, Math.max(0, (value - 0.34) / 0.12));
    const fadeOut = Math.min(1, Math.max(0, (value - 0.8) / 0.16));
    return fadeIn * (1 - 0.5 * fadeOut);
  },
};

let motion = fallbackMotion;
let animationFrame = 0;
let playing = false;

async function loadMotionModule() {
  const moduleUrl = document.documentElement.dataset.motionModule;
  document.documentElement.dataset.motionRuntime = "fallback";
  if (!("WebAssembly" in window) || !moduleUrl) return;

  try {
    const response = await fetch(moduleUrl);
    if (!response.ok) return;
    const { instance } = await WebAssembly.instantiate(
      await response.arrayBuffer(),
      {},
    );
    motion = instance.exports;
    document.documentElement.dataset.motionRuntime = "wasm";
  } catch {
    motion = fallbackMotion;
  }
}

function finishHandoff(instant = false) {
  const pathLength = progressPath.getTotalLength();
  progressPath.style.transition = instant ? "none" : "opacity 260ms ease-out";
  progressPath.style.strokeDasharray = pathLength;
  progressPath.style.strokeDashoffset = "0";
  progressPath.style.opacity = "0.34";
  signal.setAttribute("r", "0");
  check.style.opacity = "1";
  check.style.strokeDasharray = "26";
  check.style.strokeDashoffset = "0";
  midpoint.style.opacity = "0.5";
  endpoint.style.opacity = "1";
  playing = false;
}

function playHandoff() {
  if (playing) return;
  cancelAnimationFrame(animationFrame);

  if (reduceMotion.matches) {
    finishHandoff(true);
    return;
  }

  playing = true;
  const pathLength = progressPath.getTotalLength();
  const startedAt = performance.now();
  const duration = 2000;

  progressPath.style.transition = "none";
  progressPath.style.opacity = "1";
  progressPath.style.strokeDasharray = pathLength;
  progressPath.style.strokeDashoffset = pathLength;
  signal.setAttribute("r", "0");
  check.style.opacity = "0";
  check.style.strokeDasharray = "26";
  check.style.strokeDashoffset = "26";
  midpoint.style.opacity = "0";
  endpoint.style.opacity = "0";

  const step = (now) => {
    const raw = Math.min(1, (now - startedAt) / duration);
    const eased = motion.ease_out_cubic(raw);
    progressPath.style.strokeDashoffset = pathLength * (1 - eased);

    const point = progressPath.getPointAtLength(pathLength * eased);
    signal.setAttribute("cx", point.x);
    signal.setAttribute("cy", point.y);
    signal.setAttribute("r", motion.signal_radius(eased));
    midpoint.style.opacity = motion.midpoint_opacity(eased);

    if (raw < 1) {
      animationFrame = requestAnimationFrame(step);
      return;
    }

    check.style.transition = "stroke-dashoffset 240ms cubic-bezier(.22, 1, .36, 1)";
    check.style.opacity = "1";
    check.style.strokeDashoffset = "0";
    endpoint.style.transition = "opacity 260ms ease-out 60ms";
    endpoint.style.opacity = "1";
    finishHandoff();
  };

  animationFrame = requestAnimationFrame(step);
}

async function copyAgentPrompt() {
  const templateId = copyButton.dataset.copyPrompt;
  const promptTemplate = document.querySelector(`#${templateId}`);
  const prompt = promptTemplate?.content.textContent.trim();
  if (!prompt) return;

  try {
    await navigator.clipboard.writeText(prompt);
  } catch {
    const input = document.createElement("textarea");
    input.value = prompt;
    input.setAttribute("readonly", "");
    input.style.position = "fixed";
    input.style.opacity = "0";
    document.body.append(input);
    input.select();
    document.execCommand("copy");
    input.remove();
  }

  copyButton.classList.add("is-copied");
  copyButton.setAttribute("aria-label", "Prompt copied");
  playHandoff();
  window.setTimeout(() => {
    copyButton.classList.remove("is-copied");
    copyButton.setAttribute("aria-label", "Copy prompt for your agent");
  }, 1800);
}

async function loadProjectStatus() {
  const statusCopy = document.querySelector("#status-copy");

  try {
    const response = await fetch("./status.json", { cache: "no-store" });
    if (!response.ok) return;
    const status = await response.json();
    statusCopy.textContent = "Now — " + status.current + " · " + status.next;
  } catch {
    // The semantic HTML already contains the complete fallback state.
  }
}

async function loadRepositoryActivity() {
  const activity = document.querySelector("#repository-activity");
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 3500);

  try {
    const response = await fetch(
      "https://api.github.com/repos/heurema/goalrail-rs/commits?per_page=1",
      {
        headers: { Accept: "application/vnd.github+json" },
        signal: controller.signal,
      },
    );
    if (!response.ok) return;

    const [latest] = await response.json();
    const committedAt = latest?.commit?.committer?.date;
    const commitUrl = latest?.html_url;
    if (
      !committedAt
      || !commitUrl?.startsWith("https://github.com/heurema/goalrail-rs/commit/")
    ) return;

    const formatted = new Intl.DateTimeFormat("en", {
      month: "short",
      day: "numeric",
      year: "numeric",
    }).format(new Date(committedAt));
    activity.textContent = "Main updated " + formatted;
    activity.href = commitUrl;
  } catch {
    // Public GitHub activity is an enhancement; the stable link remains useful.
  } finally {
    window.clearTimeout(timeout);
  }
}

copyButton.addEventListener("click", copyAgentPrompt);
handoff.addEventListener("mouseenter", playHandoff);
handoff.addEventListener("focus", playHandoff);
reduceMotion.addEventListener("change", () => {
  cancelAnimationFrame(animationFrame);
  finishHandoff(true);
});

await loadMotionModule();
playHandoff();
loadProjectStatus();
loadRepositoryActivity();
