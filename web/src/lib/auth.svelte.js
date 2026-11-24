// Shared auth state. api.js sets needed=true on any 401; LoginSheet clears it after a successful login.
export const auth = $state({ needed: false });
