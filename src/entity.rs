#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transform {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    pub id: EntityId,
    pub transform: Transform,
}
