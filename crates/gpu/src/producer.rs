//! GPU-resident picture producers. Hosts own policy; the compositor owns targets.
use crate::Core;

/// Draw premultiplied RGBA into the supplied target on the compositor's device.
/// Implementations are created lazily and may keep GPU resources between frames.
/// Sizes are bounded by the compositor; no window/surface access is exposed.
pub trait PictureProducer: Send {
    /// Optional dynamic pixel size, clamped by the compositor.
    fn size(&self) -> Option<(u32,u32)> { None }
    fn draw(&mut self, device: &wgpu::Device, queue: &wgpu::Queue,
        view: &wgpu::TextureView, format: wgpu::TextureFormat, size: (u32,u32));
}
impl Core {
    pub fn set_gpu_picture(&mut self, id:u32, producer:Option<(Box<dyn PictureProducer>,(u32,u32))>) {
        match producer {Some(p)=>{self.producers.insert(id,p);},None=>{self.producers.remove(&id);if self.pictures.get(&id).is_some_and(|p|p.target) { if let Some(p)=self.pictures.remove(&id) {self.picture_bytes-=p.bytes;} }}}
    }
    pub fn render_gpu_pictures(&mut self) {
        let mut producers=std::mem::take(&mut self.producers);
        for (&id,(producer,size)) in &mut producers {
            let requested=producer.size().unwrap_or(*size);
            let size=(requested.0.clamp(1,4096),requested.1.clamp(1,4096));
            self.target_texture(id,size,0,0,true);
            if let Some(picture)=self.pictures.get(&id) {
                producer.draw(&self.device,&self.queue,&picture.view,self.view_format,size);
            }
        }
        self.producers=producers;
    }
}
