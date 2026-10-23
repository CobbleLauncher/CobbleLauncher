// SPDX-License-Identifier: MIT
import { describe,expect,it,vi } from 'vitest';
import { createCloseGuard,type CloseRequest } from './window-close';

// Mirrors the installed Tauri wrapper: await handler, destroy only when not prevented.
function wrapper(confirm:()=>Promise<boolean>) {
  const destroy=vi.fn();const error=vi.fn();const handle=createCloseGuard(confirm,error);
  return {destroy,error,async close() {let prevented=false;const event:CloseRequest={preventDefault:()=>{prevented=true;}};await handle(event);if(!prevented) destroy();}};
}
describe('window close request',()=>{
  it('lets the wrapper destroy a clean window once without recursively requesting close',async()=>{
    const app=wrapper(async()=>true);await app.close();expect(app.destroy).toHaveBeenCalledTimes(1);expect(app.error).not.toHaveBeenCalled();
  });
  it('keeps a dirty window open when leave is refused',async()=>{
    const app=wrapper(async()=>false);await app.close();expect(app.destroy).not.toHaveBeenCalled();
  });
  it('lets the wrapper destroy after accepting dirty changes',async()=>{
    const confirm=vi.fn(async()=>true);const app=wrapper(confirm);await app.close();expect(confirm).toHaveBeenCalledTimes(1);expect(app.destroy).toHaveBeenCalledTimes(1);
  });
  it('deduplicates repeated clicks while the confirmation is pending',async()=>{
    let accept!:(value:boolean)=>void;const confirm=vi.fn(()=>new Promise<boolean>(resolve=>{accept=resolve;}));const app=wrapper(confirm);
    const first=app.close();await app.close();expect(confirm).toHaveBeenCalledTimes(1);expect(app.destroy).not.toHaveBeenCalled();accept(true);await first;expect(app.destroy).toHaveBeenCalledTimes(1);
  });
  it('prevents destruction and reports a failed guard, then allows a later retry',async()=>{
    const confirm=vi.fn<()=>Promise<boolean>>().mockRejectedValueOnce(new Error('synthetic')).mockResolvedValueOnce(true);const app=wrapper(confirm);
    await app.close();expect(app.destroy).not.toHaveBeenCalled();expect(app.error).toHaveBeenCalledTimes(1);await app.close();expect(app.destroy).toHaveBeenCalledTimes(1);
  });
});
