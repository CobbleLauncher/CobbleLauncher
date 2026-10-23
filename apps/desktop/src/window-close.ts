// SPDX-License-Identifier: MIT
export interface CloseRequest { preventDefault(): void }

/** Tauri destroys the window after this handler unless it prevents the event. */
export function createCloseGuard(confirmLeave:()=>Promise<boolean>, reportError:()=>void) {
  let pending=false;
  return async (event:CloseRequest):Promise<void> => {
    if(pending) {event.preventDefault();return;}
    pending=true;
    try {if(!await confirmLeave()) event.preventDefault();}
    catch {event.preventDefault();reportError();}
    finally {pending=false;}
  };
}
