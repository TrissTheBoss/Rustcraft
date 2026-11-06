#[cfg(not(windows))]
use voxel_engine::render::{
    mesh_chunk, Camera, CpuRenderer, DirectionalLight, FrameProfiler, Material, MaterialRegistry,
    Renderer,
};
#[cfg(not(windows))]
use voxel_engine::world::{BlockId, ChunkPos, World, CHUNK_SIZE};

fn main() {
    #[cfg(windows)]
    {
        voxel_engine::window::run();
    }
    #[cfg(not(windows))]
    run_headless();
}

#[cfg(not(windows))]
fn run_headless() {
    let mut world = World::new();
    let chunk = world.get_or_create_chunk(ChunkPos { x: 0, y: 0, z: 0 });
    for y in 0..4 {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                chunk
                    .set(x, y, z, BlockId(if y == 3 { 2 } else { 1 }))
                    .unwrap();
            }
        }
    }
    let mesh = mesh_chunk(chunk, |x, y, z| {
        x >= 0
            && y >= 0
            && z >= 0
            && x < 16
            && y < 16
            && z < 16
            && chunk.get(x as usize, y as usize, z as usize).unwrap() != BlockId(0)
    });
    let mut materials = MaterialRegistry::default();
    materials
        .register(
            BlockId(1),
            Material {
                color: [110, 160, 86],
                texture: None,
            },
        )
        .unwrap();
    materials
        .register(
            BlockId(2),
            Material {
                color: [150, 105, 66],
                texture: None,
            },
        )
        .unwrap();
    let started = std::time::Instant::now();
    let frame = CpuRenderer {
        clear_color: [92, 142, 190],
    }
    .render(
        &[&mesh],
        &Camera::default(),
        &materials,
        DirectionalLight::default(),
        320,
        200,
    )
    .unwrap();
    let mut profiler = FrameProfiler::new(8);
    profiler.record(started.elapsed());
    let visible_pixels = frame.depth.iter().filter(|depth| depth.is_finite()).count();
    println!("Voxel Engine 0.5.0 prototype");
    println!(
        "world: {} chunk, mesh: {} triangles, frame: {}x{} ({} visible pixels)",
        world.chunk_count(),
        mesh.triangle_count(),
        frame.width,
        frame.height,
        visible_pixels
    );
    println!("cpu render time: {:?}", profiler.summary().mean);
}
