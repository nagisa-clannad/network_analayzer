use std::path::Path;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

const MIGRATION_ID: &str = "0001_foundation";
const MIGRATION_SQL: &str = include_str!("../migrations/0001_foundation.sql");
pub struct Database { connection: Connection }
#[derive(Debug, Serialize)] #[serde(rename_all = "camelCase")]
pub struct InventoryRow { pub device_entity_id: String, pub sort_name: String, pub device_type: String, pub primary_ip_display: Option<String>, pub connection_state: String, pub connection_summary: String, pub status: String, pub last_seen_at: Option<String> }
#[derive(Debug, Error)] pub enum DatabaseError { #[error("database error: {0}")] Sqlite(#[from] rusqlite::Error) }
impl Database {
    pub fn in_memory() -> Result<Self, DatabaseError> { let mut value = Self { connection: Connection::open_in_memory()? }; value.migrate()?; Ok(value) }
    pub fn open(path: &Path) -> Result<Self, DatabaseError> { let mut value = Self { connection: Connection::open(path)? }; value.migrate()?; Ok(value) }
    fn migrate(&mut self) -> Result<(), DatabaseError> {
        self.connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS schema_migrations (id TEXT PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);")?;
        let applied: Option<String> = self.connection.query_row("SELECT id FROM schema_migrations WHERE id=?1", [MIGRATION_ID], |row| row.get(0)).optional()?;
        if applied.is_none() { let tx = self.connection.transaction()?; tx.execute_batch(MIGRATION_SQL)?; tx.execute("INSERT INTO schema_migrations(id) VALUES(?1)", [MIGRATION_ID])?; tx.commit()?; }
        Ok(())
    }
    pub fn create_project(&mut self, name: &str, scope_kind: &str, scope_target: &str) -> Result<String, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let transaction = self.connection.transaction()?;
        transaction.execute("INSERT INTO entity_registry(id,entity_type) VALUES(?1,'project')", [&id])?;
        transaction.execute("INSERT INTO projects(entity_id,display_name) VALUES(?1,?2)", params![id, name.trim()])?;
        
        let profile_id = Uuid::new_v4().to_string();
        let scope_id = Uuid::new_v4().to_string();
        transaction.execute(
            "INSERT INTO discovery_scopes(id, project_id, scope_kind, target, enabled) VALUES(?1, ?2, ?3, ?4, 1)",
            params![scope_id, id, scope_kind, scope_target.trim()],
        )?;

        transaction.execute(
            "INSERT INTO scan_profiles(id, project_id, name, read_only, active_probe_enabled) VALUES(?1, ?2, 'Standard Discovery', 1, 0)",
            [&profile_id, &id]
        )?;

        transaction.commit()?;
        Ok(id)
    }
    pub fn get_default_profile_id(&self, project_id: &str) -> Result<Option<String>, DatabaseError> {
        use rusqlite::OptionalExtension;
        let id: Option<String> = self.connection.query_row(
            "SELECT id FROM scan_profiles WHERE project_id = ?1 LIMIT 1",
            [project_id],
            |row| row.get(0)
        ).optional()?;
        Ok(id)
    }
    pub fn get_profile_and_scopes(&self, profile_id: &str) -> Result<Option<(crate::core_domain::ScanProfile, Vec<crate::core_domain::DiscoveryScope>)>, DatabaseError> {
        use rusqlite::OptionalExtension;
        
        let profile: Option<(String, String, i32, i32)> = self.connection.query_row(
            "SELECT id, name, read_only, active_probe_enabled FROM scan_profiles WHERE id = ?1",
            [profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        ).optional()?;

        let Some((p_id, name, read_only, active_probe_enabled)) = profile else {
            return Ok(None);
        };

        let project_id: String = self.connection.query_row(
            "SELECT project_id FROM scan_profiles WHERE id = ?1",
            [profile_id],
            |row| row.get(0)
        )?;

        let mut stmt = self.connection.prepare(
            "SELECT id, scope_kind, target, enabled FROM discovery_scopes WHERE project_id = ?1"
        )?;
        
        let scopes_iter = stmt.query_map([&project_id], |row| {
            let id_str: String = row.get(0)?;
            let kind_str: String = row.get(1)?;
            let target: String = row.get(2)?;
            let enabled: i32 = row.get(3)?;
            
            let kind = match kind_str.as_str() {
                "cidr" => crate::core_domain::DiscoveryScopeKind::Cidr,
                "single_ip" => crate::core_domain::DiscoveryScopeKind::SingleIp,
                "seed_device" => crate::core_domain::DiscoveryScopeKind::SeedDevice,
                "site_context" => crate::core_domain::DiscoveryScopeKind::SiteContext,
                _ => crate::core_domain::DiscoveryScopeKind::Cidr,
            };
            
            let id = Uuid::parse_str(&id_str).map_err(|_| rusqlite::Error::InvalidQuery)?;

            Ok(crate::core_domain::DiscoveryScope {
                id,
                kind,
                target,
                enabled: enabled != 0,
            })
        })?;

        let mut scopes = Vec::new();
        for s in scopes_iter {
            scopes.push(s?);
        }

        let p_uuid = Uuid::parse_str(&p_id).map_err(|_| DatabaseError::Sqlite(rusqlite::Error::InvalidQuery))?;
        let scope_ids = scopes.iter().map(|s| s.id).collect();

        Ok(Some((
            crate::core_domain::ScanProfile {
                id: p_uuid,
                name,
                scope_ids,
                credential_reference_ids: Vec::new(),
                read_only: read_only != 0,
                active_probe_enabled: active_probe_enabled != 0,
            },
            scopes
        )))
    }
    pub fn inventory(&self) -> Result<Vec<InventoryRow>, DatabaseError> {
        let mut statement = self.connection.prepare("SELECT device_entity_id,sort_name,device_type,primary_ip_display,connection_state,connection_summary,status,last_seen_at FROM device_inventory_projections ORDER BY sort_name COLLATE NOCASE")?;
        let rows = statement.query_map([], |row| Ok(InventoryRow { device_entity_id: row.get(0)?, sort_name: row.get(1)?, device_type: row.get(2)?, primary_ip_display: row.get(3)?, connection_state: row.get(4)?, connection_summary: row.get(5)?, status: row.get(6)?, last_seen_at: row.get(7)? }))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::from);
        rows
    }
}
#[cfg(test)] mod tests { use super::*;
    #[test] fn initializes_a_file_database() {
        let path = std::env::temp_dir().join(format!("network-analyzer-{}.sqlite", Uuid::new_v4()));
        let database = Database::open(&path).expect("open");
        assert!(database.inventory().expect("query").is_empty());
        drop(database);
        std::fs::remove_file(&path).expect("cleanup");
        let wal_path = path.with_file_name(format!("{}-wal", path.file_name().expect("file name").to_string_lossy()));
        let shm_path = path.with_file_name(format!("{}-shm", path.file_name().expect("file name").to_string_lossy()));
        let _ = std::fs::remove_file(wal_path);
        let _ = std::fs::remove_file(shm_path);
    }

    #[test]
    fn sqlite_trigger_rejects_logical_physical_link_endpoint() {
        let database = Database::in_memory().expect("open");
        database.connection.execute_batch(
            "INSERT INTO entity_registry(id,entity_type) VALUES
                ('project','project'),('device-a','device'),('device-b','device'),
                ('if-a','interface'),('if-b','interface'),('link','physical_link');
             INSERT INTO projects(entity_id,display_name) VALUES('project','test');
             INSERT INTO devices(entity_id,project_id,display_name) VALUES
                ('device-a','project','a'),('device-b','project','b');
             INSERT INTO interfaces(entity_id,device_entity_id,name,interface_kind,physical_port_state) VALUES
                ('if-a','device-a','vlan10','vlan','logical_only'),
                ('if-b','device-b','eth0','physical','evidence');
             INSERT INTO evidence(id,project_id,target_entity_id,attribute,source_type,confidence,value_json)
                VALUES('evidence','project','if-a','link','fixture',1.0,'{}');",
        ).expect("fixture should be valid");
        let result = database.connection.execute(
            "INSERT INTO physical_links(entity_id,source_interface_entity_id,target_interface_entity_id,assertion_state,evidence_id)
             VALUES('link','if-a','if-b','known','evidence')", [],
        );
        assert!(result.is_err(), "logical interfaces must be rejected by the database trigger");
    }

    #[test]
    fn project_creation_persists_only_the_explicit_discovery_scope() {
        let mut database = Database::in_memory().expect("open");
        let project_id = database.create_project("explicit scope", "cidr", "198.51.100.0/24").expect("project");
        let profile_id = database.get_default_profile_id(&project_id).expect("profile query").expect("default profile");
        let (_, scopes) = database.get_profile_and_scopes(&profile_id).expect("profile query").expect("profile");
        assert_eq!(scopes.len(), 1);
        assert_eq!(scopes[0].target, "198.51.100.0/24");
    }
}
