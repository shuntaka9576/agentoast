type Listener<T> = (event: { payload: T }) => void;

export async function listen<T = unknown>(
  _event: string,
  _handler: Listener<T>,
): Promise<() => void> {
  return () => {};
}

export async function emit(_event: string, _payload?: unknown): Promise<void> {}
