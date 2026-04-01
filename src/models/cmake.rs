use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FindPackage {
    pub name: String,
    pub version: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetDependencies {
    pub target: String,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TargetLinks {
    pub target: String,
    pub libraries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CMakeInfo {
    pub find_packages: Vec<FindPackage>,
    pub has_catkin_package: bool,
    pub has_ament_package: bool,
    pub executables: Vec<String>,
    pub libraries: Vec<String>,
    pub ament_target_dependencies: Vec<TargetDependencies>,
    pub target_link_libraries: Vec<TargetLinks>,
}
