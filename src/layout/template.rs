use serde::Deserialize;

use super::{LayoutNode, LeafLayout, SplitChild};

#[derive(Debug, Clone)]
pub struct CountExpr(String);

impl<'de> serde::Deserialize<'de> for CountExpr {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(CountExpr(String::deserialize(deserializer)?))
    }
}

impl CountExpr {
    pub fn eval(&self, n: usize) -> usize {
        let expr = self.0.replace("N", &n.to_string());
        match calc::eval(&expr) {
            Ok(calc::Value::Integral(bigint, _)) => {
                use num_traits::cast::ToPrimitive;
                bigint.to_usize().unwrap_or(0)
            }
            Ok(calc::Value::Float(f)) => {
                f.to_string().parse::<f64>().unwrap_or(0.0).floor() as usize
            }
            Err(_) => 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayoutPreset {
    pub min_photos: usize,
    pub node: LayoutNodeTemplate,
}

#[derive(Debug, Clone, Deserialize)]
pub enum LayoutNodeTemplate {
    HSplit {
        children: Vec<SplitChildTemplate>,
    },
    VSplit {
        children: Vec<SplitChildTemplate>,
    },
    Leaf {
        layout: LeafLayout,
        count: CountExpr,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct SplitChildTemplate {
    pub weight: f32,
    pub node: LayoutNodeTemplate,
}

impl LayoutNodeTemplate {
    /// Resolve the template into a concrete LayoutNode given n photos.
    /// Returns None if any leaf would get 0 photos or total doesn't match n.
    pub fn resolve(&self, n: usize) -> Option<LayoutNode> {
        let node = self.resolve_inner(n)?;
        if node.all_leaves_valid() && node.total_photo_count() == n {
            Some(node)
        } else {
            None
        }
    }

    fn resolve_inner(&self, n: usize) -> Option<LayoutNode> {
        match self {
            LayoutNodeTemplate::Leaf { layout, count } => {
                let photo_count = count.eval(n);
                Some(LayoutNode::Leaf {
                    layout: *layout,
                    photo_count,
                })
            }
            LayoutNodeTemplate::HSplit { children } => {
                let resolved: Option<Vec<SplitChild>> = children
                    .iter()
                    .map(|c| {
                        c.node.resolve_inner(n).map(|node| SplitChild {
                            weight: c.weight,
                            node,
                        })
                    })
                    .collect();
                Some(LayoutNode::Split {
                    direction: super::SplitDirection::Horizontal,
                    children: resolved?,
                })
            }
            LayoutNodeTemplate::VSplit { children } => {
                let resolved: Option<Vec<SplitChild>> = children
                    .iter()
                    .map(|c| {
                        c.node.resolve_inner(n).map(|node| SplitChild {
                            weight: c.weight,
                            node,
                        })
                    })
                    .collect();
                Some(LayoutNode::Split {
                    direction: super::SplitDirection::Vertical,
                    children: resolved?,
                })
            }
        }
    }
}

impl LayoutPreset {
    pub fn resolve(&self, n: usize) -> Option<LayoutNode> {
        if n < self.min_photos {
            return None;
        }
        self.node.resolve(n)
    }
}

pub fn load_presets(ron_str: &str) -> Vec<LayoutPreset> {
    ron::from_str(ron_str).expect("Failed to parse layout presets RON")
}
