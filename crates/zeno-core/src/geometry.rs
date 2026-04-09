#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    #[must_use]
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Transform2D {
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    #[must_use]
    pub const fn translation(x: f32, y: f32) -> Self {
        Self {
            tx: x,
            ty: y,
            ..Self::identity()
        }
    }

    #[must_use]
    pub const fn scale(x: f32, y: f32) -> Self {
        Self {
            m11: x,
            m12: 0.0,
            m21: 0.0,
            m22: y,
            tx: 0.0,
            ty: 0.0,
        }
    }

    #[must_use]
    pub fn rotation_degrees(degrees: f32) -> Self {
        let radians = degrees.to_radians();
        let sin = radians.sin();
        let cos = radians.cos();
        Self {
            m11: cos,
            m12: sin,
            m21: -sin,
            m22: cos,
            tx: 0.0,
            ty: 0.0,
        }
    }

    #[must_use]
    pub const fn is_identity(self) -> bool {
        self.m11 == 1.0
            && self.m12 == 0.0
            && self.m21 == 0.0
            && self.m22 == 1.0
            && self.tx == 0.0
            && self.ty == 0.0
    }

    #[must_use]
    pub fn then(self, next: Self) -> Self {
        next.multiply(self)
    }

    #[must_use]
    pub fn multiply(self, rhs: Self) -> Self {
        Self {
            m11: self.m11 * rhs.m11 + self.m21 * rhs.m12,
            m12: self.m12 * rhs.m11 + self.m22 * rhs.m12,
            m21: self.m11 * rhs.m21 + self.m21 * rhs.m22,
            m22: self.m12 * rhs.m21 + self.m22 * rhs.m22,
            tx: self.m11 * rhs.tx + self.m21 * rhs.ty + self.tx,
            ty: self.m12 * rhs.tx + self.m22 * rhs.ty + self.ty,
        }
    }

    #[must_use]
    pub fn map_point(self, point: Point) -> Point {
        Point::new(
            self.m11 * point.x + self.m21 * point.y + self.tx,
            self.m12 * point.x + self.m22 * point.y + self.ty,
        )
    }

    #[must_use]
    pub fn map_rect(self, rect: Rect) -> Rect {
        let top_left = self.map_point(rect.origin);
        let top_right = self.map_point(Point::new(rect.right(), rect.origin.y));
        let bottom_left = self.map_point(Point::new(rect.origin.x, rect.bottom()));
        let bottom_right = self.map_point(Point::new(rect.right(), rect.bottom()));
        let left = top_left
            .x
            .min(top_right.x)
            .min(bottom_left.x)
            .min(bottom_right.x);
        let top = top_left
            .y
            .min(top_right.y)
            .min(bottom_left.y)
            .min(bottom_right.y);
        let right = top_left
            .x
            .max(top_right.x)
            .max(bottom_left.x)
            .max(bottom_right.x);
        let bottom = top_left
            .y
            .max(top_right.y)
            .max(bottom_left.y)
            .max(bottom_right.y);
        Rect::new(left, top, right - left, bottom - top)
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::identity()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    #[must_use]
    pub fn right(&self) -> f32 {
        self.origin.x + self.size.width
    }

    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.origin.y + self.size.height
    }

    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        let left = self.origin.x.min(other.origin.x);
        let top = self.origin.y.min(other.origin.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Self::new(left, top, right - left, bottom - top)
    }

    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        !(self.right() <= other.origin.x
            || other.right() <= self.origin.x
            || self.bottom() <= other.origin.y
            || other.bottom() <= self.origin.y)
    }
}
