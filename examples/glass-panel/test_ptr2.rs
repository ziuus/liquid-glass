use wayland_client::{protocol::wl_surface::WlSurface, Proxy, Connection};
use wayland_backend::sys::client::ObjectId as SysObjectId;

fn test(surface: &WlSurface) {
    let _ptr = SysObjectId::from(surface.id()).as_ptr();
}
