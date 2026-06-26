// gpu_timer.rs — GPU timestamp instrumentation extracted from lib.rs.
//
// GpuTimer owns the wgpu query-set + buffers used for per-frame GPU timing.
// It is created once in PlutoniumEngine::new_async() and its methods are called
// from within render() with &Device / &mut CommandEncoder / &Queue parameters
// rather than &self references to PlutoniumEngine, avoiding split-borrow walls.
//
// Fields are pub(crate) so that render() in lib.rs can access them directly
// for field-level borrow splitting (Rust allows borrowing different fields of
// a struct concurrently; calling &self methods would lock the whole struct).

use crate::utils::FrameTimeMetrics;

struct PendingGpuReadback {
    rx: std::sync::mpsc::Receiver<bool>,
    start: u64,
    end: u64,
}

pub(crate) struct GpuTimer {
    pub(crate) query: Option<wgpu::QuerySet>,
    pub(crate) buf: Option<wgpu::Buffer>,
    pub(crate) staging: Option<wgpu::Buffer>,
    pending_readback: Option<PendingGpuReadback>,
    pub(crate) period_ns: f32,
    pub(crate) count: u32,
    pub(crate) frame_index: u32,
    pub(crate) metrics: FrameTimeMetrics,
}

impl GpuTimer {
    /// Create a GpuTimer. If the device supports TIMESTAMP_QUERY the query-set and
    /// both buffers are allocated once here; otherwise the Option fields are None and
    /// all per-frame paths become no-ops. Matches the inline construction in
    /// PlutoniumEngine::new_async() verbatim. No per-frame allocation.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let period_ns: f32 = queue.get_timestamp_period();
        let mut query: Option<wgpu::QuerySet> = None;
        let mut buf: Option<wgpu::Buffer> = None;
        let mut staging: Option<wgpu::Buffer> = None;
        let mut count: u32 = 0;
        if device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            // 2 queries per frame across a small ring buffer
            count = 128;
            query = Some(device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("gpu-timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count,
            }));
            buf = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu-timestamps-buffer"),
                size: (count as u64) * 8,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu-timestamps-staging"),
                size: (count as u64) * 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            staging = Some(staging_buf);
        }
        Self {
            query,
            buf,
            staging,
            pending_readback: None,
            period_ns,
            count,
            frame_index: 0,
            metrics: FrameTimeMetrics::new(600, 5.0),
        }
    }

    /// Resolve the query set into the resolve buffer. Must be called after the
    /// render pass ends and before encoder.finish(). Matches the inline resolve
    /// logic verbatim.
    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder, q0: u32, q1: u32) {
        if let (Some(qs), Some(buf)) = (self.query.as_ref(), self.buf.as_ref()) {
            let base = (((q0 as u64) * 8) / wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT)
                * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
            encoder.resolve_query_set(qs, q0..(q1 + 1), buf, base);
        }
    }

    /// Opportunistically copy resolved timestamp results and consume completed
    /// readbacks without blocking the CPU. Mapping callbacks are driven with
    /// `Maintain::Poll`; if the previous mapping is still pending this frame skips
    /// starting another readback so the staging buffer is never reused while mapped.
    pub(crate) fn readback_and_report(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        q0: u32,
    ) {
        let _ = device.poll(wgpu::Maintain::Poll);

        if let (Some(pending), Some(dst)) = (&self.pending_readback, self.staging.as_ref()) {
            match pending.rx.try_recv() {
                Ok(true) => {
                    let slice = dst.slice(pending.start..pending.end);
                    let data = slice.get_mapped_range();
                    if data.len() >= 16 {
                        let t0 = u64::from_le_bytes(data[0..8].try_into().unwrap());
                        let t1 = u64::from_le_bytes(data[8..16].try_into().unwrap());
                        if t1 > t0 {
                            let dt_ns = (t1 - t0) as f64 * (self.period_ns as f64);
                            let dt_s = (dt_ns / 1_000_000_000.0) as f32;
                            self.metrics.record(dt_s);
                            if let Some(line) = self.metrics.maybe_report() {
                                log::info!("gpu_{}", line);
                            }
                        }
                    }
                    drop(data);
                    dst.unmap();
                    self.pending_readback = None;
                }
                Ok(false) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending_readback = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    self.frame_index = self.frame_index.wrapping_add(1);
                    return;
                }
            }
        }

        if let (Some(src), Some(dst)) = (&self.buf, &self.staging) {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("copy ts"),
            });
            let base = (((q0 as u64) * 8) / wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT)
                * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
            enc.copy_buffer_to_buffer(src, base, dst, base, wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT);
            queue.submit(Some(enc.finish()));

            let start = base;
            let end = start + 16;
            let slice = dst.slice(start..end);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send(res.is_ok());
            });
            self.pending_readback = Some(PendingGpuReadback { rx, start, end });
        }

        self.frame_index = self.frame_index.wrapping_add(1);
    }
}
