export const noop = () => Promise.resolve() as Promise<never>;
export const emptyList = () => Promise.resolve([]) as Promise<never>;
export const unsub = () => () => {};
