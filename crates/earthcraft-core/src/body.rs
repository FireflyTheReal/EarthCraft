//! Bodies selected by a frontend; texture ownership stays in that frontend.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Body {
    #[default]
    Earth,
    Moon,
    Custom,
}
