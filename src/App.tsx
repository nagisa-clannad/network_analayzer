import { useState, useEffect } from "react";
import { 
  type ScopeKind, 
  validateScope, 
  initializeProject,
  getDefaultProfile, 
  scanPreflight, 
  preflightLocalNetwork,
  type PreflightReport,
  type LocalNetworkPreflight,
} from "./api";
import "./App.css";

type View = "inventory" | "topology" | "ipam" | "scope";
const navigation: ReadonlyArray<{ id: View; label: string }> = [
  { id: "inventory", label: "デバイス" }, 
  { id: "topology", label: "トポロジー" }, 
  { id: "ipam", label: "IPAM" }, 
  { id: "scope", label: "Preflight / Scope" }
];

export default function App() {
  const [view, setView] = useState<View>("inventory");
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [profileId, setProfileId] = useState<string | null>(null);
  const [preflightOpen, setPreflightOpen] = useState(false);

  const handleProjectReady = async (projectId: string) => {
    const profileRes = await getDefaultProfile(projectId);
    if (profileRes.ok && profileRes.data) setProfileId(profileRes.data.profileId);
  };

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <strong>Network Analyzer</strong>
          <span>Phase 1 foundation</span>
        </div>
        <span className="read-only-badge">読み取り専用</span>
      </header>
      
      <div className="notice" role="status">
        Discovery scan provider は未実装です。Preflight はローカル Interface の受動列挙だけを行い、認証情報・外部通信・スキャンは行いません。
      </div>
      
      <div className="workspace">
        <nav aria-label="主要画面">
          {navigation.map((item) => (
            <button 
              className={view === item.id ? "active" : ""} 
              key={item.id} 
              onClick={() => setView(item.id)}
            >
              {item.label}
            </button>
          ))}
        </nav>
        
        <section className="content">
          {view === "inventory" && (
            <Inventory 
              onOpen={() => setDrawerOpen(true)} 
              onStartScan={() => setPreflightOpen(true)}
            />
          )}
          {view === "topology" && (
            <Page 
              title="トポロジー" 
              message="物理・L2・L3 の接続は観測根拠を持つ Entity からだけ生成されます。論理 NIC から物理リンクを作成しません。" 
            />
          )}
          {view === "ipam" && (
            <Page 
              title="IPAM" 
              message="IP アドレスと割当は未観測です。IPv6 link-local は Network Context と Interface scope を伴って保存します。" 
            />
          )}
          {view === "scope" && <ScopeForm onStartPreflight={() => setPreflightOpen(true)} onProjectReady={handleProjectReady} />}
        </section>
        
        {drawerOpen && <Drawer close={() => setDrawerOpen(false)} />}
        {preflightOpen && (
          <PreflightModal 
            profileId={profileId} 
            close={() => setPreflightOpen(false)} 
          />
        )}
      </div>
    </main>
  );
}

function Inventory({ onOpen, onStartScan }: { onOpen: () => void; onStartScan: () => void }) { 
  return (
    <>
      <header className="page-header">
        <div>
          <h1>デバイス インベントリ</h1>
          <p>Provider の観測結果が確定するまで Projection は空です。</p>
        </div>
        <button className="primary-btn" onClick={onStartScan}>スキャンを開始...</button>
      </header>
      <div className="table">
        <div className="row heading">
          <span>デバイス</span>
          <span>IP</span>
          <span>接続</span>
          <span>状態</span>
        </div>
        <button className="row empty" onClick={onOpen}>
          <span>Unknown</span>
          <span>未観測</span>
          <span>Unknown — 根拠なし</span>
          <span>詳細を確認</span>
        </button>
      </div>
    </>
  ); 
}

function Page({ title, message }: { title: string; message: string }) { 
  return (
    <>
      <header className="page-header">
        <div>
          <h1>{title}</h1>
          <p>{message}</p>
        </div>
      </header>
      <div className="empty-state">
        <strong>表示できる観測結果はありません</strong>
        <p>未観測や推測を Known として扱いません。</p>
      </div>
    </>
  ); 
}

function ScopeForm({ onStartPreflight, onProjectReady }: { onStartPreflight: () => void; onProjectReady: (projectId: string) => Promise<void> }) {
  const [kind, setKind] = useState<ScopeKind>("cidr");
  const [target, setTarget] = useState("");
  const [projectName, setProjectName] = useState("");
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [localReport, setLocalReport] = useState<LocalNetworkPreflight | null>(null);
  const [localError, setLocalError] = useState<string | null>(null);
  
  const submit = async () => {
    setResult(null);
    try {
      const response = await validateScope(kind, target);
      setResult(
        response.ok 
          ? { ok: true, message: "Scope は形式上有効です。読み取り専用としてのみ保存候補にできます。" } 
          : { ok: false, message: response.error?.message ?? "Scope は検証できませんでした。" }
      );
    } catch {
      setResult({ ok: false, message: "Scope は検証できませんでした。ネットワークアクセスは行われていません。" });
    }
  };

  const inspectLocalNetwork = async () => {
    setLocalReport(null);
    setLocalError(null);
    try {
      const response = await preflightLocalNetwork();
      if (response.ok && response.data) setLocalReport(response.data);
      else setLocalError(response.error?.message ?? "LocalNetwork capability を確認できませんでした。");
    } catch {
      setLocalError("LocalNetwork capability を確認できませんでした。ネットワーク scan は行われていません。");
    }
  };

  const createProject = async () => {
    setResult(null);
    const response = await initializeProject(projectName, kind, target);
    if (response.ok && response.data) {
      setResult({ ok: true, message: "Project と明示した Scope を保存しました。" });
      await onProjectReady(response.data.projectId);
    } else {
      setResult({ ok: false, message: response.error?.message ?? "Project を保存できませんでした。" });
    }
  };
  
  return (
    <>
      <header className="page-header">
        <div>
          <h1>Preflight / Discovery Scope</h1>
          <p>入力は形式だけを確認します。ここでは接続・認証・スキャンを実行しません。</p>
        </div>
      </header>
      <div className="scope-card">
        <label>Project 名<input value={projectName} onChange={(event) => setProjectName(event.target.value)} placeholder="ローカル環境" /></label>
        <label>
          Scope 種別
          <select value={kind} onChange={(event) => setKind(event.target.value as ScopeKind)}>
            <option value="cidr">CIDR</option>
            <option value="single_ip">単一 IP</option>
            <option value="seed_device">Seed Device</option>
            <option value="site_context">Site / Network Context</option>
          </select>
        </label>
        <label>
          対象
          <input 
            value={target} 
            onChange={(event) => setTarget(event.target.value)} 
            placeholder={kind === "cidr" ? "192.0.2.0/24" : kind === "single_ip" ? "192.0.2.10" : "識別子"} 
          />
        </label>
        <div className="button-group">
          <button onClick={submit}>形式を検証</button>
          <button className="secondary-btn" onClick={onStartPreflight}>Preflight レポートを確認</button>
          <button className="secondary-btn" onClick={inspectLocalNetwork}>ローカル capability を確認</button>
          <button className="secondary-btn" onClick={createProject}>Project / Scope を保存</button>
        </div>
        {result && <p className={result.ok ? "result ok" : "result error"}>{result.message}</p>}
        {localError && <p className="result error">{localError}</p>}
        {localReport && <LocalCapabilityReport report={localReport} />}
        <p className="subtle">LocalNetwork は Interface/address の受動列挙だけを行います。route、ARP/NDP、ICMP、raw packet は unsupported です。</p>
      </div>
    </>
  );
}

function LocalCapabilityReport({ report }: { report: LocalNetworkPreflight }) {
  const label = (value: string) => value === "available" ? "利用可能" : value === "no_privilege" ? "権限不足" : "非対応";
  return <section className="local-capability" aria-label="ローカル capability 結果"><h3>{report.providerId} / {report.os}</h3><p>Provider {report.providerVersion} — Interface {report.interfaceCount} 件。これは scan 完了を意味しません。</p><div className="capability-summary"><span>Interface 列挙: {label(report.capabilities.interfaceEnumeration)}</span><span>Route: {label(report.capabilities.route)}</span><span>ARP/NDP: {label(report.capabilities.arpNdp)}</span><span>ICMP: {label(report.capabilities.icmp)}</span><span>Raw packet: {label(report.capabilities.rawPacket)}</span></div><details><summary>取得した Interface 名とアドレスを表示</summary><ul>{report.interfaces.map((item) => <li key={item.name}>{item.name} ({item.kind}, physical port: {item.physicalPortState}) — {item.ips.join(", ") || "アドレスなし"}</li>)}</ul></details></section>;
}

function Drawer({ close }: { close: () => void }) { 
  return (
    <aside className="drawer">
      <button className="close" onClick={close}>閉じる</button>
      <h2>デバイス詳細</h2>
      <p className="unknown">Unknown</p>
      <h3>Connection</h3>
      <p>接続先・種別・根拠は未観測です。</p>
      <h3>IP & NIC</h3>
      <p>物理 NIC と仮想 NIC は区別して表示します。現在は未観測です。</p>
      <h3>Evidence</h3>
      <p>Evidence はありません。</p>
    </aside>
  ); 
}

function PreflightModal({ profileId, close }: { profileId: string | null; close: () => void }) {
  const [loading, setLoading] = useState(true);
  const [report, setReport] = useState<PreflightReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function loadPreflight() {
      if (!profileId) {
        setError("Tauri desktop backend に接続されていません。Preflight は実行されませんでした。");
        setLoading(false);
        return;
      }

      try {
        const res = await scanPreflight(profileId);
        if (res.ok && res.data) {
          setReport(res.data);
        } else {
          setError(res.error?.message ?? "Preflight レポートの取得に失敗しました。");
        }
      } catch (err) {
        setError("バックエンドサーバーとの通信中にエラーが発生しました。");
      } finally {
        setLoading(false);
      }
    }
    loadPreflight();
  }, [profileId]);

  if (loading) {
    return (
      <div className="modal-overlay">
        <div className="modal-content text-center">
          <div className="spinner"></div>
          <p>Scan Preflight 情報を検証中...</p>
        </div>
      </div>
    );
  }

  if (error || !report) {
    return (
      <div className="modal-overlay">
        <div className="modal-content">
          <header className="modal-header">
            <h2>Preflight エラー</h2>
          </header>
          <div className="modal-body">
            <p className="text-danger">{error || "Preflight 情報を解決できませんでした。"}</p>
          </div>
          <footer className="modal-footer">
            <button className="primary-btn" onClick={close}>閉じる</button>
          </footer>
        </div>
      </div>
    );
  }

  const isLimitExceeded = report.estimatedTargets > report.maxHosts;

  return (
    <div className="modal-overlay" role="dialog" aria-modal="true" aria-labelledby="modal-title">
      <div className="modal-content">
        <header className="modal-header">
          <h2 id="modal-title">Scan Preflight (事前検証)</h2>
          <button className="close-btn" onClick={close} aria-label="閉じる">×</button>
        </header>
        
        <div className="modal-body">
          <section className="preflight-section">
            <h3>探索ターゲット & 規模制限</h3>
            <div className="preflight-grid">
              <div>
                <strong>探索範囲 (Target):</strong>
                <span> {report.scopeTarget}</span>
              </div>
              <div>
                <strong>スコープ種別:</strong>
                <span className="badge badge-kind"> {report.scopeKind.toUpperCase()}</span>
              </div>
              <div>
                <strong>推定ターゲットホスト数:</strong>
                <span className={isLimitExceeded ? "text-danger bold" : "bold"}> {report.estimatedTargets} ホスト</span>
              </div>
              <div>
                <strong>最大ホスト数制限:</strong>
                <span> {report.maxHosts} ホスト</span>
              </div>
            </div>
            
            {isLimitExceeded && (
              <div className="warning-box">
                <strong>⚠️ ホスト上限超過:</strong> 推定ターゲット数が上限値 ({report.maxHosts} ホスト) を超えています。スキャンを開始できません。探索スコープを縮小してください。
              </div>
            )}
          </section>

          <section className="preflight-section">
            <h3>OS 権限 & Provider Capability</h3>
            <p className="section-desc">OSのセキュリティ権限状態に応じたプロバイダー動作の判定結果です。</p>
            <div className="capability-list">
              <div className="capability-item">
                <span>ICMP Echo (Ping) 送信:</span>
                <span className={`badge cap-${report.capabilities.icmp}`}>
                  {report.capabilities.icmp === "available" ? "利用可能 (Available)" : report.capabilities.icmp === "no_privilege" ? "権限不足 (No Privilege)" : "非対応 (Unsupported)"}
                </span>
              </div>
              <div className="capability-item">
                <span>ARP/NDP テーブル取得:</span>
                <span className={`badge cap-${report.capabilities.arpNdp}`}>
                  {report.capabilities.arpNdp === "available" ? "利用可能 (Available)" : report.capabilities.arpNdp === "no_privilege" ? "権限不足 (No Privilege)" : "非対応 (Unsupported)"}
                </span>
              </div>
              <div className="capability-item">
                <span>ルーティングテーブル取得:</span>
                <span className={`badge cap-${report.capabilities.route}`}>
                  {report.capabilities.route === "available" ? "利用可能 (Available)" : report.capabilities.route === "no_privilege" ? "権限不足 (No Privilege)" : "非対応 (Unsupported)"}
                </span>
              </div>
              <div className="capability-item">
                <span>ローカルネットワーク情報 (LocalNetwork):</span>
                <span className={`badge cap-${report.capabilities.localNetwork}`}>
                  {report.capabilities.localNetwork === "available" ? "利用可能 (Available)" : report.capabilities.localNetwork === "no_privilege" ? "権限不足 (No Privilege)" : "非対応 (Unsupported)"}
                </span>
              </div>
            </div>
          </section>

          {report.skippedFeatures && report.skippedFeatures.length > 0 && (
            <section className="preflight-section">
              <h3>収集時のスキップ・警告事項</h3>
              <ul className="skipped-list">
                {report.skippedFeatures.map((msg, index) => (
                  <li key={index}><strong>※</strong> {msg}</li>
                ))}
              </ul>
            </section>
          )}
        </div>

        <footer className="modal-footer">
          <button className="secondary-btn" onClick={close}>キャンセル</button>
          <button className="primary-btn" disabled title="Discovery provider は未実装です">スキャンは Provider 実装後に有効</button>
        </footer>
      </div>
    </div>
  );
}
