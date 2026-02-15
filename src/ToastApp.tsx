import { useEffect, useState, useRef, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import type { Notification } from "@/lib/types";
import { IconPreset, TmuxIcon } from "@/components/icons/source-icon";

const DEFAULT_TOAST_DURATION = 4000;
const FADE_DURATION = 300;

const badgeColorClasses: Record<string, string> = {
  green: "bg-[var(--badge-stop-bg)] text-[var(--badge-stop-text)]",
  blue: "bg-[var(--badge-notif-bg)] text-[var(--badge-notif-text)]",
  red: "bg-red-500/20 text-red-400",
  gray: "bg-[var(--hover-bg-strong)] text-[var(--text-tertiary)]",
};

export function ToastApp() {
  const [queue, setQueue] = useState<Notification[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [isVisible, setIsVisible] = useState(false);
  const [isFadingOut, setIsFadingOut] = useState(false);
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const fadeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const toastDurationRef = useRef(DEFAULT_TOAST_DURATION);
  const toastPersistentRef = useRef(false);
  const queueRef = useRef<Notification[]>([]);
  const currentIndexRef = useRef(0);
  const isVisibleRef = useRef(false);

  useEffect(() => {
    invoke<number>("get_toast_duration")
      .then((d) => { toastDurationRef.current = d; })
      .catch(() => {});
    invoke<boolean>("get_toast_persistent")
      .then((p) => { toastPersistentRef.current = p; })
      .catch(() => {});
  }, []);

  const hideToast = useCallback(() => {
    setIsFadingOut(true);
    fadeTimerRef.current = setTimeout(() => {
      setIsVisible(false);
      isVisibleRef.current = false;
      setIsFadingOut(false);
      setQueue([]);
      setCurrentIndex(0);
      queueRef.current = [];
      currentIndexRef.current = 0;
      void invoke("hide_toast");
    }, FADE_DURATION);
  }, []);

  const startTimer = useCallback((onExpire: () => void) => {
    if (hideTimerRef.current) {
      clearTimeout(hideTimerRef.current);
    }
    if (!toastPersistentRef.current) {
      hideTimerRef.current = setTimeout(onExpire, toastDurationRef.current);
    }
  }, []);

  const advanceOrHide = useCallback(() => {
    const nextIndex = currentIndexRef.current + 1;
    if (nextIndex < queueRef.current.length) {
      setCurrentIndex(nextIndex);
      currentIndexRef.current = nextIndex;
      startTimer(() => advanceOrHide());
    } else {
      hideToast();
    }
  }, [hideToast, startTimer]);

  useEffect(() => {
    const unlisten = listen<Notification[]>("toast:show", (event) => {
      const notifications = event.payload;
      if (notifications.length === 0) return;

      // Clear any pending timers
      if (hideTimerRef.current) {
        clearTimeout(hideTimerRef.current);
        hideTimerRef.current = null;
      }
      if (fadeTimerRef.current) {
        clearTimeout(fadeTimerRef.current);
        fadeTimerRef.current = null;
      }
      setIsFadingOut(false);

      if (queueRef.current.length > 0 && isVisibleRef.current) {
        const currentIdx = currentIndexRef.current;
        const oldQueue = queueRef.current;

        // Keep only unshown notifications (from current index onward)
        const remaining = oldQueue.slice(currentIdx);

        // Remove duplicates (same group+tmuxPane as incoming) from remaining
        const remainingDeduped = remaining.filter((q) =>
          !notifications.some((n) => n.tmuxPane && q.groupName === n.groupName && q.tmuxPane === n.tmuxPane)
        );

        // LIFO: newest notifications first, then remaining unshown
        const newQueue = [...[...notifications].reverse(), ...remainingDeduped];

        setQueue(newQueue);
        setCurrentIndex(0);
        queueRef.current = newQueue;
        currentIndexRef.current = 0;
        startTimer(() => advanceOrHide());
      } else {
        // Start fresh queue (LIFO: newest first)
        const reversed = [...notifications].reverse();
        setQueue(reversed);
        setCurrentIndex(0);
        queueRef.current = reversed;
        currentIndexRef.current = 0;
        setIsVisible(true);
        isVisibleRef.current = true;
        startTimer(() => advanceOrHide());
      }
    });

    return () => {
      void unlisten.then((fn) => fn());
      if (hideTimerRef.current) {
        clearTimeout(hideTimerRef.current);
      }
      if (fadeTimerRef.current) {
        clearTimeout(fadeTimerRef.current);
      }
    };
  }, [advanceOrHide, startTimer]);

  const handleClick = () => {
    if (hideTimerRef.current) {
      clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }

    const current = queueRef.current[currentIndexRef.current];
    if (current) {
      if (!current.forceFocus) {
        void invoke("delete_notification", { id: current.id });
      }
      void invoke("focus_terminal", {
        tmuxPane: current.tmuxPane,
      });
    }

    // Advance to next or hide
    const nextIndex = currentIndexRef.current + 1;
    if (nextIndex < queueRef.current.length) {
      setCurrentIndex(nextIndex);
      currentIndexRef.current = nextIndex;
      startTimer(() => advanceOrHide());
    } else {
      hideToast();
    }
  };

  const current = queue[currentIndex];

  if (!isVisible || !current) {
    return <div className="h-screen bg-transparent" />;
  }

  const badgeClass = badgeColorClasses[current.color] || badgeColorClasses.gray;
  const metaEntries = Object.entries(current.metadata).filter(
    ([, v]) => v !== "",
  );
  return (
    <div
      className={cn(
        "h-screen p-2 bg-transparent cursor-pointer",
        isFadingOut ? "animate-toast-out" : "animate-toast-in",
      )}
      onClick={handleClick}
    >
      <div className={cn(
        "relative backdrop-blur-xl rounded-xl border shadow-2xl p-3 h-full",
        current.forceFocus
          ? "bg-[var(--toast-focus-bg)] border-[var(--border-focus)]"
          : "bg-[var(--toast-bg)] border-[var(--border-primary)]"
      )}>
        <div className="flex items-start gap-2.5">
          <div className="flex-shrink-0 mt-0.5">
            <IconPreset
              icon={current.icon}
              size={20}
              className="text-[var(--text-secondary)]"
            />
          </div>
          <div className="flex-1 min-w-0">
            {/* Title badge + group name */}
            <div className="flex items-center gap-2">
              {current.title && (
                <span
                  className={cn(
                    "px-1.5 py-0.5 text-[10px] font-medium rounded flex-shrink-0",
                    badgeClass,
                  )}
                >
                  {current.title}
                </span>
              )}
              <span className="text-[12px] font-medium text-[var(--text-primary)] truncate">
                {current.groupName}
              </span>
            </div>

            {/* Metadata */}
            {metaEntries.length > 0 && (
              <div className="flex items-center gap-1 mt-1 text-[11px] text-[var(--text-tertiary)] truncate">
                {metaEntries.map(([key, value], i) => (
                  <span key={key}>
                    {i > 0 && <span className="mx-0.5">·</span>}
                    <span className="text-[var(--text-muted)]">{key}:</span>{" "}
                    {value}
                  </span>
                ))}
              </div>
            )}

            {/* tmux info */}
            {current.tmuxPane && (
              <div className="flex items-center gap-1 mt-1 text-[11px] text-[var(--text-tertiary)] truncate">
                <TmuxIcon size={11} className="flex-shrink-0" />
                {current.tmuxPane}
              </div>
            )}

            {/* Body */}
            {current.body && (
              <p className="mt-1 text-[11px] text-[var(--text-secondary)] line-clamp-2">
                {current.body}
              </p>
            )}

          </div>
        </div>
        {/* Queue counter badge */}
        {queue.length > 1 && (
          <span className="absolute top-2 right-3 px-1.5 py-0.5 text-[10px] font-medium rounded bg-[var(--hover-bg-strong)] text-[var(--text-secondary)]">
            {currentIndex + 1}/{queue.length}
          </span>
        )}
        {/* Focused: no history badge (absolute bottom-right) */}
        {current.forceFocus && (
          <span className="absolute bottom-2 right-3 px-1.5 py-0.5 text-[10px] font-medium rounded bg-[var(--badge-focus-bg)] text-[var(--badge-focus-text)]">
            Focused: no history
          </span>
        )}
      </div>
    </div>
  );
}
