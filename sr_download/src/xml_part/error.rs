#[derive(Debug)]
pub enum XmlError {
    Io(std::io::Error),
    Deserialize(quick_xml::DeError),
    Serialize(String),
    UnsupportedRoot(String),
    UnexpectedDocumentType {
        expected: &'static str,
        found: &'static str,
    },
}

pub type XmlResult<T> = Result<T, XmlError>;

impl std::fmt::Display for XmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Deserialize(err) => write!(f, "xml deserialize error: {err}"),
            Self::Serialize(err) => write!(f, "xml serialize error: {err}"),
            Self::UnsupportedRoot(root) => write!(f, "unsupported xml root: {root}"),
            Self::UnexpectedDocumentType { expected, found } => {
                write!(
                    f,
                    "unexpected xml document type: expected {expected}, found {found}"
                )
            }
        }
    }
}

impl std::error::Error for XmlError {}

impl From<std::io::Error> for XmlError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<quick_xml::DeError> for XmlError {
    fn from(value: quick_xml::DeError) -> Self {
        Self::Deserialize(value)
    }
}

impl From<quick_xml::Error> for XmlError {
    fn from(value: quick_xml::Error) -> Self {
        Self::Serialize(value.to_string())
    }
}
