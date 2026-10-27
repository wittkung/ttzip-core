# Contract: Application Core (`app-core`) UniFFI Interface

**Package**: `com.metastudyline.ttzip.appcore`  
**Binding Targets**: Swift, C#, C++, Python  

---

## 1. Device Management Interface

```rust
#[uniffi::export]
pub trait DeviceManagerListener: Send + Sync {
    fn on_device_event(&self, event: DeviceEvent);
}

#[derive(uniffi::Enum)]
pub enum DeviceEvent {
    Attached(DeviceDescriptor),
    Detached(String), // UUID
    StateChanged(String, DeviceConnectionState),
}

#[derive(uniffi::Object)]
pub struct AppCoreDeviceManager { ... }

#[uniffi::export]
impl AppCoreDeviceManager {
    #[uniffi::constructor]
    pub fn new(listener: Box<dyn DeviceManagerListener>) -> Self;

    pub fn list_devices(&self) -> Vec<DeviceDescriptor>;

    pub fn list_storage_tree(
        &self,
        device_uuid: String,
        partition_id: String,
        parent_path: String,
    ) -> Result<Vec<StorageItem>, AppCoreError>;

    pub fn start_transfer(
        &self,
        job: TransferJobConfig,
    ) -> Result<String, AppCoreError>;

    pub fn pause_transfer(&self, job_id: String) -> Result<(), AppCoreError>;

    pub fn resume_transfer(&self, job_id: String) -> Result<(), AppCoreError>;

    pub fn cancel_transfer(&self, job_id: String) -> Result<(), AppCoreError>;
}
```

---

## 2. Plugin PKI Verification Interface

```rust
#[derive(uniffi::Object)]
pub struct AppCorePluginVerifier { ... }

#[uniffi::export]
impl AppCorePluginVerifier {
    #[uniffi::constructor]
    pub fn new() -> Self;

    pub fn verify_manifest_and_bundle(
        &self,
        manifest_json: String,
        bundle_tar_path: String,
    ) -> Result<VerifiedPluginPackage, SecurityError>;
}

#[derive(uniffi::Record)]
pub struct VerifiedPluginPackage {
    pub plugin_id: String,
    pub developer_id: String,
    pub authorized_capabilities: Vec<PluginCapability>,
    pub package_checksum: String,
}
```
