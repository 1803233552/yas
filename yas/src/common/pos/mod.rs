mod ops;
mod scale;

use std::ops::*;

pub use ops::*;
pub use scale::*;

#[derive(Debug, Clone, PartialEq, Default, Copy)]
pub struct Pos<T = i32> {
    pub x: T,
    pub y: T,
}

#[derive(Debug, Clone, PartialEq, Default, Copy)]
pub struct Size<T = u32> {
    pub width: T,
    pub height: T,
}

#[derive(Debug, Clone, PartialEq, Default, Copy)]
pub struct Rect<P = i32, S = u32> {
    pub origin: Pos<P>,
    pub size: Size<S>,
}

#[derive(Debug, Clone, PartialEq, Default, Copy)]
pub struct RectBound<T = i32> {
    pub left: T,
    pub top: T,
    pub right: T,
    pub bottom: T,
}

impl<T> Pos<T> {
    pub fn new(x: T, y: T) -> Pos<T> {
        Pos { x, y }
    }
}

impl<T> Size<T> {
    pub fn new(width: T, height: T) -> Size<T> {
        Size { width, height }
    }

    pub fn area(&self) -> T
    where
        T: Mul<Output = T> + Copy,
    {
        self.width * self.height
    }
}

impl<P, S> Rect<P, S> {
    pub fn new(x: P, y: P, width: S, height: S) -> Rect<P, S> {
        Rect {
            origin: Pos::new(x, y),
            size: Size::new(width, height),
        }
    }
}

impl<T> Deref for Size<T>
where
    T: Copy,
{
    type Target = (T, T);

    fn deref(&self) -> &Self::Target {
        &(self.width, self.height)
    }
}

impl<T> Deref for Pos<T>
where
    T: Copy,
{
    type Target = (T, T);

    fn deref(&self) -> &Self::Target {
        &(self.x, self.y)
    }
}

impl<T> RectBound<T>
where
    T: PartialOrd,
{
    pub fn new(left: T, top: T, right: T, bottom: T) -> RectBound<T> {
        if left > right || top > bottom {
            panic!("Invalid bound value");
        }

        RectBound {
            left,
            top,
            right,
            bottom,
        }
    }
}

impl<I, U> From<Rect<I, U>> for RectBound<I>
where
    I: PartialOrd + Add<I, Output = I>,
    U: PartialOrd + Into<I>,
{
    fn from(rect: Rect<I, U>) -> RectBound<I> {
        RectBound::new(
            rect.origin.x,
            rect.origin.y,
            rect.origin.x + rect.size.width.into(),
            rect.origin.y + rect.size.height.into(),
        )
    }
}

impl<I, U> From<RectBound<I>> for Rect<I, U>
where
    I: PartialOrd + Sub<I, Output = I> + Into<U>,
    U: PartialOrd,
{
    fn from(bound: RectBound<I>) -> Rect<I, U> {
        if bound.left > bound.right || bound.top > bound.bottom {
            panic!("Invalid bound value");
        }

        Rect::new(
            bound.left,
            bound.top,
            (bound.right - bound.left).into(),
            (bound.bottom - bound.top).into(),
        )
    }
}

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use core_graphics::geometry::*;

    impl From<CGPoint> for Pos {
        fn from(point: CGPoint) -> Pos {
            Pos::new(point.x as i32, point.y as i32)
        }
    }

    impl From<CGSize> for Size {
        fn from(size: CGSize) -> Size {
            Size::new(size.width as u32, size.height as u32)
        }
    }

    impl From<CGRect> for Rect {
        fn from(rect: CGRect) -> Rect {
            Rect {
                origin: Pos::from(rect.origin),
                size: Size::from(rect.size),
            }
        }
    }

    impl Rect {
        pub fn with_titlebar(mut self, height: i32) -> Self {
            self.origin.y += height;
            self.size.height -= height as u32;
            self
        }
    }
}
