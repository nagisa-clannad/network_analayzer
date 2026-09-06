# ネットワーク構成自動可視化・IPAMツール DB設計 v1.1

- 対象: 統合仕様書 v0.4.7 の Phase 1
- DBMS: SQLite（Project 単位の DB ファイル）
- 正本: `Observation` とその根拠から作る `Evidence`
- 表示用: 正規化 Entity と Current State projection

## 1. 設計方針

```text
Provider result
  -> immutable observations / diagnostics
  -> evidence / inference
  -> normalized entities + current-state projection
  -> diagrams / IPAM / changes / unresolved
```

- **観測履歴を上書きしない。** Provider は `observations` と `provider_diagnostics` を追記するだけであり、Entity や Diagram を直接更新しない。
- **Current State はキャッシュ兼 projection である。** Entity の現在値は Evidence から導出し、完了した ScanSession ごとに transaction で更新する。途中 Session は Diff baseline に使わない。
- **秘密情報を保存しない。** DB は OS Secure Storage の参照情報だけを持つ。community、password、token、private key は DB、JSON export、backup manifest、log に入れない。
- **ID は UUIDv7 を TEXT で保存する。** 時系列順に近い ID を使い、外部に露出しても連番を推測されないようにする。
- **時刻は UTC RFC 3339（小数秒、`Z`）の TEXT とする。** SQLite 上で文字列順比較できる canonical 表現だけを許可する。
- **enum は TEXT で保存する。** CHECK 制約で将来値を閉じない。アプリ側で `unknown` / `other` と未知値を安全に扱う。

## 2. SQLite の運用設定

Project 作成時と各接続時に次を適用する。

```sql
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA busy_timeout = 5000;
PRAGMA trusted_schema = OFF;
```

- アプリケーションプロセス内の writer は Project DB ごとに一つとする。Reader は WAL snapshot を使う。
- SQL は全て prepared statement を用いる。SQL 文字列を Observation、hostname、LLDP 値、UI filter から組み立てない。
- DB file、`-wal`、`-shm`、backup を同じユーザーだけが読める OS ACL のディレクトリに置く。
- 起動時に `quick_check`、migration / restore 前後に `integrity_check` を実行する。失敗時は read-only recovery mode で開き、Scan を開始しない。

## 3. 型・命名規約

| 種別 | SQLite 型 / 規約 |
| --- | --- |
| ID | `TEXT NOT NULL`。UUIDv7。列名は `*_id`。 |
| boolean | `INTEGER NOT NULL`、`0` / `1`。 |
| IP | `address_family INTEGER`（4 / 6）、`address_binary BLOB`（4 / 16 byte）、`address_display TEXT`。文字列だけで比較しない。 |
| MAC | `normalized_mac TEXT`、uppercase・区切りなしの 12 hex。 |
| JSON | `TEXT`。metadata、Profile snapshot、raw payload のように relational query を要求しない値だけに限定する。JSON 配列で外部キーを表現しない。 |
| nullable | 未観測と「値なし」を区別する。`NULL` は未観測、明示された否定は enum / boolean / Evidence で表す。 |
| lifecycle | `archived_at` で soft archive。通常 Scan で物理 DELETE しない。 |

すべての状態テーブルは、少なくとも `created_at`、`updated_at`、`first_seen_at`、`last_seen_at` のうち意味を持つ列を持つ。時刻をアプリの locale で保存してはならない。

## 4. スキーマの全体像

```text
projects ──< sites ──< network_contexts
    │                         │
    │                         └──< entity_registry
    │                                  ├── devices ──< network_adapters ──< interfaces
    │                                  ├── mac_addresses
    │                                  ├── vlans / subnets / ip_addresses
    │                                  ├── physical_links / shared_segments
    │                                  └── routes / wan_circuits
    │
    ├──< discovery_scopes / credential_references / scan_sessions
    │                                      ├──< observations / provider_diagnostics
    │                                      └──< scan_session_events
    │
    ├──< evidences ──< evidence_observations
    ├──< inferences ──< inference_evidences
    ├──< user_overrides / audit_events / unresolved_items
    ├──< diagrams ──< diagram_nodes
    └──< change_sets ──< change_items
```

`entity_registry` は Evidence、Override、Diagram、Audit が任意種別の Entity を参照するための共通親テーブルである。具体テーブルは `entity_id` を primary key かつ foreign key とし、Evidence に polymorphic ID を直接持たせない。

## 5. Project、Scope、Credential、Scan

### 5.1 Project と Context

| テーブル | 主な列 | 制約 |
| --- | --- | --- |
| `projects` | `id`, `name`, `description`, `default_timezone`, `created_at`, `archived_at` | `name` は Project 内で表示名。global unique にしない。 |
| `sites` | `id`, `project_id`, `name`, `location`, `timezone`, `tags_json` | `UNIQUE(project_id, name)` は任意。移設履歴は Audit に残す。 |
| `network_contexts` | `id`, `project_id`, `site_id?`, `name`, `tenant`, `namespace`, `vrf_name?`, `description` | `UNIQUE(project_id, site_id, name, namespace)`。同名を強制統合しない。 |
| `entity_registry` | `id`, `project_id`, `entity_kind`, `created_at`, `archived_at` | `(id, project_id)` を候補 key とし、子テーブルの Project 混在を防ぐ。 |

### 5.2 Scope と Profile

| テーブル | 主な列 | 制約 |
| --- | --- | --- |
| `discovery_scopes` | `id`, `project_id`, `name`, `active_discovery_allowed`, `max_hosts`, `max_depth`, `approved_at`, `approved_by` | Scope を無効化できる。削除ではなく archive。 |
| `scope_targets` | `id`, `scope_id`, `rule_kind`, `target_value`, `is_exclude` | CIDR / IP / seed / context を canonical 化して保存。重複 rule を禁止。 |
| `discovery_profiles` | `id`, `project_id?`, `name`, `profile_kind`, `version`, `settings_json`, `is_builtin` | built-in は immutable。TCP port、timeout、rate limit を versioned settings に保持。 |
| `scope_profile_bindings` | `scope_id`, `profile_id`, `enabled` | `(scope_id, profile_id)` を一意。 |

### 5.3 Credential reference

| テーブル | 主な列 | 制約 |
| --- | --- | --- |
| `credential_references` | `id`, `project_id`, `protocol`, `scope_kind`, `scope_entity_id?`, `label`, `secret_store_key`, `enabled`, `validation_state`, `last_validated_at` | secret value を持たない。`validation_state` は `unknown` / `available` / `secret_missing` / `authentication_failed` / `disabled`。 |

- `secret_store_key` は OS Secure Storage 内の opaque key であり、export 対象外とする。
- restore / open 時に secret の実体を照合する。実体がなければ `secret_missing` とし、参照する Scope の Preflight / Scan を `credential_required` で停止する。
- Scope 解決は Device > Subnet > Site > Project の順に**一つだけ**選ぶ。候補を順に試す credential fallback は実装しない。

### 5.4 Scan と raw data

| テーブル | 主な列 | 制約 |
| --- | --- | --- |
| `scan_sessions` | `id`, `project_id`, `scope_snapshot_json`, `profile_snapshot_json`, `provider_versions_json`, `seed_snapshot_json`, `status`, `started_at`, `ended_at`, `baseline_session_id?`, `coverage_summary_json`, `cancellation_reason?` | Project ごとの `status='running'` は partial unique index で一つに制限。 |
| `scan_session_events` | `id`, `session_id`, `sequence`, `event_kind`, `payload_json`, `created_at` | `UNIQUE(session_id, sequence)`。UI 再接続の補助。 |
| `observations` | `id`, `session_id`, `sequence`, `provider_id`, `provider_version`, `scope_id`, `raw_kind`, `normalized_kind`, `target_json`, `value_json`, `payload_hash`, `collected_at`, `observed_at`, `received_at`, `result`, `diagnostic_code?` | `UNIQUE(session_id, sequence)`。追記専用。secret をマスク後に保存。 |
| `observation_payloads` | `observation_id`, `masked_payload_json`, `retention_expires_at` | raw payload を保存する必要がある場合だけ作る。保持切れで DELETE 可能。 |
| `provider_diagnostics` | `id`, `session_id`, `provider_id`, `target_json`, `code`, `severity`, `message`, `metadata_json`, `created_at` | `message` に secret / raw command を入れない。 |

プロセス異常終了後に `running` の Session は `interrupted` へ遷移させる。`failed`、`cancelled`、`interrupted` の Session は Current State、Missing、Removed の根拠に使わない。

## 6. Evidence、推論、手動変更

| テーブル | 主な列 | 制約 |
| --- | --- | --- |
| `evidences` | `id`, `project_id`, `subject_entity_id`, `attribute_name`, `value_json`, `source_type`, `confidence`, `assertion_state`, `observed_at`, `expires_at`, `metadata_json` | subject は `entity_registry` を FK で参照。Evidence 本体は更新しない。 |
| `evidence_observations` | `evidence_id`, `observation_id` | 複数 Observation を一つの Evidence の根拠にできる。両列で primary key。 |
| `inferences` | `id`, `project_id`, `subject_entity_id`, `attribute_name`, `value_json`, `rule_id`, `rule_version`, `confidence`, `generated_at`, `superseded_at?` | 同じ rule を再実行しても旧推論を破棄しない。 |
| `inference_evidences` | `inference_id`, `evidence_id` | 推論の入力根拠を正規化して保存。 |
| `user_overrides` | `id`, `project_id`, `subject_entity_id`, `attribute_name`, `value_json`, `reason`, `created_by`, `created_at`, `revoked_at?`, `revoked_by?`, `revoke_reason?` | UPDATE で自動値を置換しない。Undo は revoke event。 |
| `audit_events` | `id`, `project_id`, `actor`, `event_kind`, `subject_entity_id?`, `before_json?`, `after_json?`, `reason?`, `correlation_id`, `created_at` | 監査目的で append-only。secret と raw payload を拒否する validator を通す。 |

Current State の優先順位は `active UserOverride > UserConfirmed Evidence > unexpired Evidence > current Inference > fallback` とする。期限切れ Evidence は消さず、projection 側で `stale` とする。

## 7. 正規化されたネットワーク Entity

### 7.1 Device と NIC

| テーブル | 主な列 | key / 制約 |
| --- | --- | --- |
| `devices` | `entity_id`, `network_context_id?`, `site_id?`, `canonical_name`, `hostname?`, `device_type`, `vendor?`, `model?`, `os_name?`, `status`, `is_virtual`, `first_seen_at`, `last_seen_at` | `entity_id` PK / FK。name や IP を unique にしない。 |
| `device_identities` | `id`, `device_entity_id`, `identity_type`, `canonical_value`, `raw_value`, `confidence`, `first_seen_at`, `last_seen_at`, `active` | 同じ identity を複数 Device に保持可能。衝突は Unresolved にする。 |
| `mac_addresses` | `entity_id`, `normalized_mac`, `is_locally_administered`, `is_multicast`, `oui_prefix?`, `oui_vendor?`, `first_seen_at`, `last_seen_at` | `UNIQUE(normalized_mac)`。Device / VLAN 関係はここへ持たない。 |
| `network_adapters` | `entity_id`, `device_entity_id`, `canonical_name`, `stable_os_id?`, `adapter_kind`, `mac_entity_id?`, `admin_status`, `oper_status`, `carrier_status`, `media_type`, `link_speed_bps?`, `maximum_speed_bps?`, `mtu?`, `parent_adapter_entity_id?`, `virtual_switch_name?`, `virtualization_platform?`, `bus_type?`, `bus_location?`, `hardware_vendor?`, `hardware_model?`, `serial_number?`, `driver_name?`, `driver_version?`, `firmware_version?`, `source_visibility`, `first_seen_at`, `last_seen_at` | `stable_os_id` は `device_entity_id` 内だけで unique。remote SNMP 由来の PCI / driver 等は NULL + Evidence reason。 |
| `interfaces` | `entity_id`, `device_entity_id`, `network_adapter_entity_id?`, `name`, `canonical_name`, `if_index?`, `interface_kind`, `physical_port_state`, `mac_entity_id?`, `parent_interface_entity_id?`, `admin_status`, `oper_status`, `speed_bps?`, `mtu?`, `first_seen_at`, `last_seen_at` | `if_index` が non-null の間は `UNIQUE(device_entity_id, if_index)`。interface kind は物理可否の唯一の根拠にしない。 |

`physical_port_state` は `physical_capable` / `logical_only` / `unknown` とする。`physical_links` の endpoint は両方 `physical_capable` かつ対応 Evidence がある Interface に限定する。

### 7.2 L2 と Physical topology

| テーブル | 主な列 | key / 制約 |
| --- | --- | --- |
| `physical_links` | `entity_id`, `source_interface_entity_id`, `target_interface_entity_id`, `media_type?`, `speed_bps?`, `duplex?`, `link_status`, `discovery_method`, `assertion_state`, `confidence`, `first_seen_at`, `last_seen_at` | endpoint を canonical order で保存し、self link を禁止。`logical_only` endpoint を拒否する trigger / service validation を置く。 |
| `shared_segments` | `entity_id`, `network_context_id?`, `name`, `assertion_state`, `confidence`, `first_seen_at`, `last_seen_at` | unknown L2 中間部分。 |
| `shared_segment_members` | `segment_entity_id`, `interface_entity_id`, `membership_role`, `first_seen_at`, `last_seen_at` | FDB / ARP から直接 PhysicalLink を作らない。 |
| `lags` | `entity_id`, `device_entity_id`, `name`, `protocol`, `first_seen_at`, `last_seen_at` | logical LAG。 |
| `lag_members` | `lag_entity_id`, `interface_entity_id`, `member_state` | `(lag_entity_id, interface_entity_id)` PK。 |
| `device_groups` | `entity_id`, `group_type`, `logical_device_entity_id?`, `name` | Stack、virtual chassis、HA pair の拡張境界。 |
| `device_group_members` | `group_entity_id`, `member_device_entity_id`, `member_role` | 同じ Device の重複 membership を禁止。 |
| `vlans` | `entity_id`, `network_context_id`, `site_id?`, `vlan_id`, `name?`, `status`, `first_seen_at`, `last_seen_at` | `UNIQUE(network_context_id, site_id, vlan_id)`。 |
| `vlan_memberships` | `vlan_entity_id`, `interface_entity_id`, `tagging`, `pvid`, `is_native`, `allowed_state` | current membership。履歴は Evidence / Observation。 |
| `link_vlan_memberships` | `physical_link_entity_id`, `vlan_entity_id`, `tagging`, `is_native` | trunk の Link 論理情報。 |
| `mac_observations` | `id`, `mac_entity_id`, `source_device_entity_id`, `source_interface_entity_id`, `vlan_entity_id?`, `ip_entity_id?`, `first_seen_at`, `last_seen_at` | MAC と port の時系列関係。 |

### 7.3 L3 / IPAM / DNS

| テーブル | 主な列 | key / 制約 |
| --- | --- | --- |
| `subnets` | `entity_id`, `network_context_id`, `address_family`, `network_binary`, `prefix_length`, `network_display`, `status`, `description?`, `first_seen_at`, `last_seen_at` | `UNIQUE(network_context_id, address_family, network_binary, prefix_length)`。全 IP を生成しない。 |
| `ip_addresses` | `entity_id`, `network_context_id`, `address_family`, `address_binary`, `address_display`, `link_scope_key`, `scope_zone?`, `status`, `is_temporary`, `first_seen_at`, `last_seen_at` | global / ULA は空文字 `link_scope_key`、link-local は non-empty。`UNIQUE(network_context_id, address_family, address_binary, link_scope_key)`。 |
| `ip_assignments` | `id`, `ip_entity_id`, `device_entity_id?`, `interface_entity_id?`, `mac_entity_id?`, `assignment_type`, `valid_from`, `valid_to?`, `is_active`, `confidence` | 同一 IP の同時複数割当を許容し、Conflict Candidate と Evidence で判断する。 |
| `ip_pools` | `entity_id`, `subnet_entity_id`, `name`, `start_binary`, `end_binary`, `pool_type`, `description?` | range は Subnet 内であることを service validation。 |
| `vlan_subnet_relations` | `vlan_entity_id`, `subnet_entity_id`, `assertion_state`, `confidence` | VLAN と Subnet を同一 table にしない。 |
| `dns_records` | `id`, `network_context_id`, `ip_entity_id?`, `record_type`, `name`, `target_value`, `ttl_seconds?`, `first_seen_at`, `last_seen_at` | A / AAAA / PTR の current projection。DNS が唯一の DeviceIdentity ではない。 |

### 7.4 Route、WAN、Wireless

| テーブル | 主な列 | key / 制約 |
| --- | --- | --- |
| `routes` | `entity_id`, `network_context_id`, `device_entity_id`, `address_family`, `destination_binary`, `prefix_length`, `next_hop_ip_entity_id?`, `outgoing_interface_entity_id?`, `protocol`, `metric?`, `administrative_distance?`, `vrf_name?`, `is_active`, `first_seen_at`, `last_seen_at` | route が PhysicalLink を作ることはない。 |
| `wan_circuits` | `entity_id`, `device_entity_id`, `interface_entity_id?`, `provider?`, `circuit_id?`, `access_type?`, `bandwidth_down_bps?`, `bandwidth_up_bps?`, `status`, `role`, `cost?`, `priority?`, `first_seen_at`, `last_seen_at` | WAN role は route と分離。 |
| `wireless_networks` | `entity_id`, `site_id?`, `ssid`, `security_type?`, `vlan_entity_id?`, `network_context_id?` | SSID と BSSID を分離。 |
| `wireless_radios` | `entity_id`, `device_entity_id`, `interface_entity_id?`, `band`, `channel?`, `channel_width?`, `frequency_mhz?`, `tx_power_dbm?` | 有線 PhysicalLink にしない。 |
| `bssids` | `entity_id`, `wireless_network_entity_id`, `radio_entity_id?`, `mac_entity_id?`, `channel?`, `status` | 同一 SSID の複数 AP を許容。 |
| `wireless_associations` | `id`, `client_device_entity_id?`, `client_interface_entity_id?`, `client_mac_entity_id?`, `bssid_entity_id`, `vlan_entity_id?`, `rssi_dbm?`, `snr_db?`, `connected_at`, `last_seen_at`, `status` | roaming は時系列 association として扱う。 |

## 8. Change、Unresolved、Diagram

| テーブル | 主な列 | key / 制約 |
| --- | --- | --- |
| `change_sets` | `id`, `project_id`, `baseline_session_id`, `comparison_session_id`, `comparability_state`, `coverage_summary_json`, `created_at` | completed / comparable Session のみで作る。 |
| `change_items` | `id`, `change_set_id`, `subject_entity_id?`, `change_kind`, `before_json?`, `after_json?`, `state`, `suppression_reason?` | credential failure、Scope 縮小を removed として保存しない。 |
| `unresolved_items` | `id`, `project_id`, `subject_entity_id?`, `unresolved_kind`, `state`, `recurrence_key`, `first_seen_at`, `last_seen_at`, `disposition_reason?`, `disposition_at?`, `disposition_by?` | state は `open` / `resolved` / `dismissed` / `deferred` / `recurred`。`recurrence_key` で再発を同一項目として追う。 |
| `unresolved_evidences` | `unresolved_item_id`, `evidence_id` | unresolved の根拠 snapshot。 |
| `diagrams` | `id`, `project_id`, `name`, `view_type`, `network_context_id?`, `filters_json`, `created_at`, `updated_at` | Network Data を複製しない。 |
| `diagram_nodes` | `diagram_id`, `entity_id`, `x`, `y`, `is_collapsed`, `is_hidden`, `updated_at` | `(diagram_id, entity_id)` PK。位置だけを保存。 |
| `diagram_groups` | `id`, `diagram_id`, `name`, `x`, `y`, `width`, `height`, `is_collapsed` | 表示用 group。Entity の親子関係にしない。 |
| `device_inventory_projections` | `device_entity_id`, `project_id`, `network_context_id?`, `sort_name`, `device_type`, `vendor_display?`, `primary_ip_entity_id?`, `primary_ip_display?`, `connection_state`, `connection_summary`, `connection_target_entity_id?`, `status`, `last_seen_at`, `refreshed_at` | Device Inventory 専用の導出 projection。`device_entity_id` PK。Evidence を複製せず、Drawer / Evidence Inspector への Entity ID を保持する。 |

## 9. Current State の更新手順

Scan 完了時に Application Service が次の transaction を実行する。

1. Session status が `completed` または `completed_with_warnings` であることを確認する。
2. Scope / Profile / Provider coverage と baseline の比較可能性を計算する。
3. Observation から Evidence を追加し、期限・confidence・unknown reason を更新する。
4. Evidence / Inference / UserOverride の優先順位で Device、NIC、Interface、L2/L3/IPAM の current projection を upsert する。続けて `device_inventory_projections` を再計算し、Connection の `known` / `inferred` / `unknown` と表示文字列を確定する。
5. Unresolved と ChangeSet を作る。ただし coverage reduced、credential failure、permission denied、timeout だけで Missing / Removed を作らない。
6. transaction を commit してから Session event `completed` を発行する。

commit に失敗した場合、Session は `failed` とし、前回の Current State を保持する。Provider が得た Observation は調査用に残してよいが Diff baseline にしない。

## 10. Index と query budget

Phase 1 で最低限作る index は次の通り。

```text
scan_sessions(project_id, status, started_at DESC)
observations(session_id, sequence)
observations(provider_id, collected_at DESC)
evidences(subject_entity_id, attribute_name, expires_at DESC)
devices(entity_id, last_seen_at DESC)
device_identities(canonical_value, identity_type, active)
interfaces(device_entity_id, if_index)
network_adapters(device_entity_id, adapter_kind, last_seen_at DESC)
mac_observations(mac_entity_id, vlan_entity_id, last_seen_at DESC)
ip_addresses(network_context_id, address_family, address_binary, link_scope_key)
ip_assignments(ip_entity_id, is_active, valid_from DESC)
vlans(network_context_id, site_id, vlan_id)
physical_links(source_interface_entity_id, target_interface_entity_id)
change_items(change_set_id, change_kind, state)
unresolved_items(project_id, state, unresolved_kind, last_seen_at DESC)
diagram_nodes(diagram_id, entity_id)
device_inventory_projections(project_id, network_context_id, sort_name, last_seen_at DESC)
device_inventory_projections(project_id, connection_state, status, last_seen_at DESC)
```

一覧 API は必ず cursor pagination を使う。Device、IP、MAC、VLAN、Subnet の横断検索は最初は正規化列の prefix / exact search とし、日本語全文検索や任意メタデータの全文検索は、tokenizer と個人・運用情報の扱いを確定してから追加する。

## 11. Retention、backup、migration

### 11.1 Retention

- `observations` の raw payload は既定 30 日。`observation_payloads` を先に削除し、必要に応じて Observation 本体を retention policy に従って purge する。
- Evidence、Inference、Audit、ChangeSet、UserOverride は調査・再現に必要なため、Phase 1 では明示的な archive policy なしに削除しない。
- purge は Scan と同時に full `VACUUM` を実行しない。incremental vacuum の結果を記録し、full `VACUUM` は backup と空き容量を確認した保守操作だけで許可する。

### 11.2 Backup / restore

- backup は SQLite の consistent snapshot を用い、DB、schema version、作成時刻、integrity check 結果を manifest に記録する。
- Secure Storage の秘密値は backup に含めない。restore 後は Credential reference を照合し、`secret_missing` を解決するまで Scan を禁止する。
- restore は新しい DB file に検証復元してから切替える。既存 Project DB を上書きする restore は、利用者が明示的に選び、直前 backup を作る。

### 11.3 Migration

`schema_migrations(id, checksum, applied_at, app_version)` を append-only で管理する。migration は単一 transaction で実行し、SQLite で transaction 外を要する操作は migration 前 backup と明示的な recovery 手順を持つ。

| migration | 内容 |
| --- | --- |
| `0001_core` | metadata、Project / Site / Context、entity registry、Audit。 |
| `0002_scan` | Scope / Profile / Credential reference、ScanSession、Observation、Diagnostic。 |
| `0003_evidence` | Evidence、Inference、Override、retention。 |
| `0004_inventory` | Device、MAC、NetworkAdapter、Interface、Identity。 |
| `0005_topology_ipam` | L2、PhysicalLink、Subnet、IP、Route、WAN。 |
| `0006_ui_state` | Diagram、Change、Unresolved。 |

## 12. DB 受入条件

1. `PRAGMA foreign_keys=ON` で全ての FK が成立し、Project を跨ぐ Entity 参照を保存できない。
2. 同一 Project で `running` ScanSession が二つ作れず、crash recovery 後の `interrupted` Session が Current State を変更しない。
3. Observation、Evidence、Inference の各根拠を UI から一意にたどれ、raw payload purge 後は参照不能理由を表示できる。
4. MAC、IP、identity conflict、link-local IPv6、ifIndex 再利用、LAG member を含む fixture で一意性・多対多・時系列制約を検証する。
5. logical-only Interface を endpoint とする PhysicalLink insert が service validation と DB trigger の両方で拒否される。
6. DB、export、backup manifest、全ログから secret test value を検索して 0 件であることを検査する。
7. fresh create、全 migration upgrade、migration failure、backup/restore、retention purge、integrity failure recovery を自動試験する。
