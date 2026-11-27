/** 한 번에 하나만 보내고, 기다리는 동안 들어온 값은 마지막 것만 남긴다. */
export function latestOnly<T>(send: (value: T) => Promise<unknown>) {
  let next: { value: T } | undefined;
  let running: Promise<void> | undefined;
  const drain = async () => {
    // 마지막 전송이 실패했을 때만 알린다. 실패해도 기다리던 값은 보낸다.
    let failed: { error: unknown } | undefined;
    while (next) {
      const { value } = next;
      next = undefined;
      try {
        await send(value);
        failed = undefined;
      } catch (error) {
        failed = { error };
      }
    }
    running = undefined;
    if (failed) throw failed.error;
  };
  return (value: T): Promise<void> => {
    next = { value };
    return (running ??= drain());
  };
}
