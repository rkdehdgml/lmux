//! 화면 배치. 모양은 `src/layout.ts`의 `Node`와 같다.

use serde::{Deserialize, Serialize};

const MAX_PANES: usize = 4;
/// `src/layout.ts`의 `MIN_RATIO`와 같다.
const MIN_RATIO: f64 = 0.15;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    Row,
    Col,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Node {
    Pane {
        session: Option<String>,
    },
    Split {
        dir: Dir,
        ratio: f64,
        first: Box<Node>,
        second: Box<Node>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SavedLayout {
    pub id: String,
    pub name: String,
    pub tree: Node,
}

impl Node {
    /// 화면이 그릴 수 있는 배치인지 확인한다.
    pub fn check(&self) -> Result<(), String> {
        fn panes(node: &Node) -> Option<usize> {
            match node {
                Node::Pane { .. } => Some(1),
                Node::Split {
                    ratio,
                    first,
                    second,
                    ..
                } => (MIN_RATIO..=1.0 - MIN_RATIO)
                    .contains(ratio)
                    .then(|| Some(panes(first)? + panes(second)?))
                    .flatten(),
            }
        }
        match panes(self) {
            Some(count) if count <= MAX_PANES => Ok(()),
            _ => Err("저장할 수 없는 화면 배치예요".into()),
        }
    }
}
