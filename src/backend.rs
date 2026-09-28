//! 実行先 backend の識別。picker などの表示 index ではなく、永続化と
//! 順番待ち(orchestration)が共有する安定した値。
use serde::{Deserialize, Serialize};

/// mock::Scenario の永続化キー。配列の順序は Scenario 列挙と
/// UI の SCENARIOS 一覧に一致させる。v1 ストアの `Mock{scenario: usize}` は
/// この順序を index として読み、キーへ移行する。
pub const MOCK_SCENARIO_KEYS: &[&str] = &[
    "demo",
    "threads",
    "events-10k",
    "events-100k",
    "log-100mib",
    "faults",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendKind {
    Subscription,
    Acp {
        id: String,
    },
    /// 疑似シナリオの安定キー(MOCK_SCENARIO_KEYS の値)。
    Mock {
        key: String,
    },
}

impl BackendKind {
    /// v1 は scenario: usize(index)。未知 index・未知キーは None。
    fn mock_key(index: Option<usize>, key: Option<String>) -> Option<String> {
        match (index, key) {
            (Some(index), None) => MOCK_SCENARIO_KEYS.get(index).map(|key| (*key).to_owned()),
            (None, Some(key)) if !key.is_empty() => Some(key),
            _ => None,
        }
    }
}

/// デシリアライズだけの中間形。`Mock` は新旧両キーを受けて key へ正規化する。
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum BackendKindRepr {
    Subscription,
    Acp {
        id: String,
    },
    Mock {
        #[serde(default)]
        scenario: Option<usize>,
        #[serde(default)]
        key: Option<String>,
    },
}

impl<'de> Deserialize<'de> for BackendKind {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        match BackendKindRepr::deserialize(de)? {
            BackendKindRepr::Subscription => Ok(Self::Subscription),
            BackendKindRepr::Acp { id } => Ok(Self::Acp { id }),
            BackendKindRepr::Mock { scenario, key } => Self::mock_key(scenario, key)
                .map(|key| Self::Mock { key })
                .ok_or_else(|| serde::de::Error::custom("Mock backend のシナリオ識別がありません")),
        }
    }
}
