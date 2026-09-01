import { useCallback, useEffect, useState, type ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { BellIcon, CheckCircledIcon, ClockIcon, DashboardIcon, GearIcon, LockClosedIcon, MixerHorizontalIcon, PaperPlaneIcon, PlusIcon, RocketIcon } from "@radix-ui/react-icons";
import { api } from "./api";
import i18n from "./i18n";
import { Badge, BrandMark, Button, Card, Field, Switch } from "./components";
import type { ApiError, AppSettings, AppState, Diagnostics, EventRecord, EventStatus, ProjectRule } from "./types";

type Page="home"|"delivery"|"rules"|"system";

export function App(){
  const {t,i18n}=useTranslation();
  const [page,setPage]=useState<Page>("home");
  const [state,setState]=useState<AppState|null>(null);
  const [settings,setSettings]=useState<AppSettings|null>(null);
  const [events,setEvents]=useState<EventRecord[]>([]);
  const [error,setError]=useState("");
  const [notice,setNotice]=useState("");
  const [loading,setLoading]=useState(true);

  const load=useCallback(async()=>{try{const next=await api.state();setState(next);setSettings(next.settings);setError("")}catch(value){setError(message(value,t("unknownError")))}finally{setLoading(false)}},[t]);
  const loadEvents=useCallback(async(status?:EventStatus)=>{try{setEvents(await api.events(status))}catch(value){setError(message(value,t("unknownError")))}},[t]);
  const showNotice=useCallback((value:string)=>{setNotice(value);window.setTimeout(()=>setNotice(""),2200)},[]);

  useEffect(()=>{void load();void loadEvents()},[load,loadEvents]);
  useEffect(()=>{const refresh=()=>{if(document.visibilityState!=="visible")return;void load();if(page==="home")void loadEvents()};const timer=window.setInterval(refresh,60_000);document.addEventListener("visibilitychange",refresh);return()=>{window.clearInterval(timer);document.removeEventListener("visibilitychange",refresh)}},[load,loadEvents,page]);
  useEffect(()=>{if(!settings)return;const theme=settings.theme==="system"?(matchMedia("(prefers-color-scheme: dark)").matches?"dark":"light"):settings.theme;document.documentElement.classList.toggle("dark",theme==="dark")},[settings?.theme]);

  const update=<K extends keyof AppSettings>(key:K,value:AppSettings[K])=>setSettings(current=>current?{...current,[key]:value}:current);
  const save=async(extra?:Partial<AppSettings>)=>{if(!settings)return;const next={...settings,...extra};try{await api.save(next);setSettings(next);if(next.language==="system"){localStorage.removeItem("language");await i18n.changeLanguage(navigator.language.startsWith("zh")?"zh":"en")}else{await i18n.changeLanguage(next.language);localStorage.setItem("language",next.language)}showNotice(t("saved"));await load()}catch(value){setError(message(value,t("unknownError")))}};
  if(loading)return <div className="loading"><BrandMark className="loading-mark"/><span>CodexNotify</span></div>;
  if(!settings||!state)return <div className="fatal"><BrandMark className="fatal-mark"/><h1>CodexNotify</h1><p>{error}</p><Button kind="primary" onClick={()=>void load()}>Retry</Button></div>;

  const nav:[Page,string,string,ReactElement][]=[
    ["home",t("overview"),t("overviewDesc"),<DashboardIcon/>],
    ["delivery",t("delivery"),t("deliveryDesc"),<PaperPlaneIcon/>],
    ["rules",t("rules"),t("rulesDesc"),<MixerHorizontalIcon/>],
    ["system",t("system"),t("systemDesc"),<GearIcon/>],
  ];
  const current=nav.find(item=>item[0]===page)!;
  const ready=state.hook.installed&&state.hook.trusted&&state.hook.enabled&&state.hook.pathCurrent&&state.secrets.barkKeyConfigured;
  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><BrandMark className="brand-mark"/><div><strong>CodexNotify</strong><span>Native Bark relay</span></div></div>
      <nav>{nav.map(([key,label,,icon])=><button key={key} className={page===key?"active":""} onClick={()=>setPage(key)}>{icon}<span>{label}</span></button>)}</nav>
      <div className={`agent-state ${ready?"ready":"attention"}`}><span className="state-light"/><div><strong>{ready?t("healthy"):t("needsAttention")}</strong><small>{ready?t("allConnected"):state.hook.reviewRequired?t("untrusted"):t("finishSetup")}</small></div></div>
      <span className="version">CODEXNOTIFY · 1.0</span>
    </aside>
    <main>
      <header className="topbar"><div><h1>{current[1]}</h1><p>{current[2]}</p></div><div className="header-actions"><Switch checked={settings.enabled} onChange={value=>update("enabled",value)} label={t("enabled")}/><Button kind="primary" onClick={()=>void save()}>{t("save")}</Button></div></header>
      {error&&<div className="toast error" onClick={()=>setError("")}>{error}</div>}{notice&&<div className="toast success">{notice}</div>}
      <div className="page-content">
        {page==="home"&&<Home state={state} events={events} refresh={loadEvents} onError={setError} onNotice={showNotice} onTest={async()=>{try{await api.test();showNotice(t("testSent"))}catch(value){setError(message(value,t("unknownError")))}}} onNavigate={setPage}/>}
        {page==="delivery"&&<Delivery settings={settings} update={update} secrets={state.secrets} onRefresh={load} onError={setError}/>}
        {page==="rules"&&<Rules settings={settings} update={update}/>}
        {page==="system"&&<SystemPage settings={settings} update={update} state={state} onRefresh={load} onError={setError} onNotice={showNotice}/>}
      </div>
    </main>
    {!settings.setupCompleted&&<Setup settings={settings} update={update} onError={setError} finish={async(key)=>{try{await api.save(settings);await api.setSecret("barkKey",key);await api.test();await api.installHook();await api.save({...settings,setupCompleted:true});setSettings({...settings,setupCompleted:true});await load()}catch(value){setError(message(value,t("unknownError")))}}}/>}
  </div>;
}

function Home({state,events,refresh,onError,onNotice,onTest,onNavigate}:{state:AppState;events:EventRecord[];refresh:(status?:EventStatus)=>Promise<void>;onError:(value:string)=>void;onNotice:(value:string)=>void;onTest:()=>void;onNavigate:(page:Page)=>void}){
  const{t}=useTranslation();
  const hookReady=state.hook.installed&&state.hook.trusted&&state.hook.enabled&&state.hook.pathCurrent;
  const ready=hookReady&&state.secrets.barkKeyConfigured;
  const pending=state.counts.queued+state.counts.retrying+state.counts.sending;
  return <div className="stack">
    <section className={`readiness ${ready?"is-ready":"needs-setup"}`}><div className="readiness-copy"><span className="readiness-icon">{ready?<CheckCircledIcon/>:<ClockIcon/>}</span><div><span className="kicker">{t("deliveryStatus")}</span><h2>{ready?t("deliveryReady"):t("setupRequired")}</h2><p>{ready?t("deliveryReadyBody"):t("setupRequiredBody")}</p></div></div><div className="button-row"><Button onClick={onTest}>{t("test")}</Button>{!ready&&<Button kind="primary" onClick={()=>onNavigate(hookReady?"delivery":"system")}>{t("resolve")}</Button>}</div></section>
    <div className="status-strip"><StatusItem label={t("codexHook")} value={hookReady?t("connected"):state.hook.reviewRequired?t("untrusted"):t("notInstalled")} ok={hookReady}/><StatusItem label={t("barkConnection")} value={state.secrets.barkKeyConfigured?t("configured"):t("notConfigured")} ok={state.secrets.barkKeyConfigured}/><StatusItem label={t("pendingQueue")} value={String(pending)} ok={pending===0}/><StatusItem label={t("delivered")} value={String(state.counts.sent)} ok/></div>
    <ActivityPanel events={events} refresh={refresh} onError={onError} onNotice={onNotice}/>
  </div>;
}

function StatusItem({label,value,ok}:{label:string;value:string;ok:boolean}){return <div className="status-item"><span>{label}</span><strong>{value}</strong><i className={ok?"ok":"warn"}/></div>}

function ActivityPanel({events,refresh,onError,onNotice}:{events:EventRecord[];refresh:(status?:EventStatus)=>Promise<void>;onError:(value:string)=>void;onNotice:(value:string)=>void}){
  const{t}=useTranslation();const[filter,setFilter]=useState<string>("");
  const action=async(operation:()=>Promise<unknown>,success:string)=>{try{await operation();await refresh(filter as EventStatus||undefined);onNotice(success)}catch(value){onError(message(value,t("unknownError")))}};
  return <Card title={t("activity")} action={<div className="button-row"><Button onClick={()=>void action(api.retryFailed,t("retryQueued"))}>{t("retryAll")}</Button><Button kind="danger" onClick={()=>void action(api.clearHistory,t("historyCleared"))}>{t("clear")}</Button></div>}><div className="activity-toolbar"><div className="segmented">{[["",t("all")],["failed",t("failed")],["suppressed",t("suppressed")]].map(([value,label])=><button className={filter===value?"active":""} key={value} onClick={()=>{setFilter(value);void refresh(value as EventStatus||undefined)}}>{label}</button>)}</div><span>{events.length} {t("records")}</span></div><EventList events={events} retry={id=>void action(()=>api.retry(id),t("retryQueued"))}/></Card>;
}

function EventList({events,retry}:{events:EventRecord[];retry:(id:number)=>void}){const{t}=useTranslation();if(!events.length)return <div className="empty-state"><BellIcon/><strong>{t("noEvents")}</strong><span>{t("noEventsHint")}</span></div>;return <div className="event-list">{events.map(event=><div className="event-row" key={event.id}><span className={`event-dot ${event.status}`}/><div className="event-main"><strong>{event.project||"Codex"}</strong><span>{event.subtitle||event.eventType}</span>{event.error&&<small>{event.error}</small>}</div><time>{new Date(event.createdAt*1000).toLocaleString()}</time><Badge tone={event.status}>{t(`event${event.status[0].toUpperCase()}${event.status.slice(1)}`)}</Badge>{event.status==="failed"&&<Button onClick={()=>retry(event.id)}>{t("retry")}</Button>}</div>)}</div>}

function Delivery({settings,update,secrets,onRefresh,onError}:{settings:AppSettings;update:<K extends keyof AppSettings>(key:K,value:AppSettings[K])=>void;secrets:AppState["secrets"];onRefresh:()=>Promise<void>;onError:(value:string)=>void}){
  const{t}=useTranslation();const[barkKey,setBarkKey]=useState("");const[encryptionKey,setEncryptionKey]=useState("");
  const saveSecret=async(kind:string,value:string)=>{try{await api.setSecret(kind,value);kind==="barkKey"?setBarkKey(""):setEncryptionKey("");await onRefresh()}catch(error){onError(message(error,t("unknownError")))}};
  return <div className="content-grid"><div className="stack"><Card title={t("connection")}><div className="form-grid"><Field label={t("barkServer")} hint={t("barkServerHint")}><input value={settings.barkServer} onChange={event=>update("barkServer",event.target.value)}/></Field><SecretField label={t("deviceKey")} configured={secrets.barkKeyConfigured} value={barkKey} onChange={setBarkKey} onSave={()=>void saveSecret("barkKey",barkKey)} onDelete={()=>void api.deleteSecret("barkKey").then(onRefresh).catch(error=>onError(message(error,t("unknownError"))))}/><Field label={t("group")}><input value={settings.group} onChange={event=>update("group",event.target.value)}/></Field><Field label={t("level")}><select value={settings.level} onChange={event=>update("level",event.target.value)}><option value="active">Active</option><option value="timeSensitive">Time sensitive</option><option value="passive">Passive</option><option value="critical">Critical</option></select></Field><Field label={t("sound")}><input value={settings.sound} onChange={event=>update("sound",event.target.value)}/></Field><Field label={t("requestTimeout")}><input type="number" min="2" max="30" value={settings.requestTimeout} onChange={event=>update("requestTimeout",Number(event.target.value))}/></Field></div></Card>
    <Card title={t("message")}><div className="form-grid"><Field label={t("titleTemplate")}><input value={settings.notificationTitle} onChange={event=>update("notificationTitle",event.target.value)}/></Field><Field label={t("bodyMode")}><select value={settings.messageMode} onChange={event=>update("messageMode",event.target.value)}><option value="minimal">Minimal</option><option value="fixed">Fixed</option><option value="summary200">Summary · 200</option><option value="summary500">Summary · 500</option><option value="full">Full</option></select></Field>{settings.messageMode==="fixed"&&<Field label={t("fixedMessage")}><input value={settings.fixedMessage} onChange={event=>update("fixedMessage",event.target.value)}/></Field>}<Field label={t("retryLimit")}><input type="number" min="1" max="8" value={settings.retryLimit} onChange={event=>update("retryLimit",Number(event.target.value))}/></Field><Field label="Bark icon URL"><input value={settings.barkIcon} onChange={event=>update("barkIcon",event.target.value)}/></Field><Field label="Click URL"><input value={settings.clickUrl} onChange={event=>update("clickUrl",event.target.value)}/></Field></div></Card>
    <Card title={t("encryption")}><Switch checked={settings.encryptionEnabled} onChange={value=>update("encryptionEnabled",value)} label={t("encryption")}/>{settings.encryptionEnabled&&<div className="form-grid inset-fields"><Field label={t("algorithm")}><select value={settings.encryptionAlgorithm} onChange={event=>update("encryptionAlgorithm",event.target.value)}><option>AES-128-CBC</option><option>AES-256-CBC</option></select></Field><SecretField label={t("encryptionKey")} configured={secrets.encryptionKeyConfigured} value={encryptionKey} onChange={setEncryptionKey} onSave={()=>void saveSecret("encryptionKey",encryptionKey)} onDelete={()=>void api.deleteSecret("encryptionKey").then(onRefresh).catch(error=>onError(message(error,t("unknownError"))))}/></div>}</Card></div>
    <Card title={t("preview")} className="preview-card"><div className="preview-stage"><div className="preview-device"><div className="preview-bar"><span>9:41</span><span>•••</span></div><div className="notification-preview"><BrandMark className="preview-logo"/><div><small>CODEXNOTIFY · NOW</small><strong>{settings.notificationTitle.replace("{project}","CodexNotify")}</strong><p>{settings.messageMode==="fixed"?settings.fixedMessage:t("previewBody")}</p></div></div></div></div><p className="security-note"><LockClosedIcon/>{t("secretSafety")}</p></Card></div>;
}

function SecretField({label,configured,value,onChange,onSave,onDelete}:{label:string;configured:boolean;value:string;onChange:(value:string)=>void;onSave:()=>void;onDelete:()=>void}){const{t}=useTranslation();return <Field label={label} hint={configured?t("configured"):t("notConfigured")}><div className="input-action"><input type="password" value={value} placeholder={configured?"••••••••":""} onChange={event=>onChange(event.target.value)}/><Button onClick={onSave} disabled={!value}>{t("saveSecret")}</Button>{configured&&<Button kind="danger" onClick={onDelete}>{t("remove")}</Button>}</div></Field>}

function Rules({settings,update}:{settings:AppSettings;update:<K extends keyof AppSettings>(key:K,value:AppSettings[K])=>void}){
  const{t}=useTranslation();const setProject=(index:number,patch:Partial<ProjectRule>)=>update("projects",settings.projects.map((project,current)=>current===index?{...project,...patch}:project));
  return <div className="content-grid rules-grid"><div className="stack"><Card title={t("projectScope")}><div className="segmented wide">{[["all",t("allProjects")],["include",t("includeProjects")],["exclude",t("excludeProjects")]].map(([value,label])=><button key={value} className={settings.scope===value?"active":""} onClick={()=>update("scope",value)}>{label}</button>)}</div></Card><Card title={t("projectRules")} action={<Button onClick={()=>update("projects",[...settings.projects,{path:"",name:"",enabled:true}])}><PlusIcon/>{t("addProject")}</Button>}><div className="project-list">{settings.projects.length?settings.projects.map((project,index)=><div className="project-row" key={`${index}-${project.path}`}><Switch checked={project.enabled} onChange={value=>setProject(index,{enabled:value})} label=""/><input placeholder={t("path")} value={project.path} onChange={event=>setProject(index,{path:event.target.value})}/><input placeholder={t("displayName")} value={project.name} onChange={event=>setProject(index,{name:event.target.value})}/><Button kind="danger" onClick={()=>update("projects",settings.projects.filter((_,current)=>current!==index))}>{t("remove")}</Button></div>):<div className="empty-state compact"><strong>{t("allProjectsEnabled")}</strong><span>{t("addRulesHint")}</span></div>}</div></Card></div>
    <div className="stack"><Card title={t("quietHours")}><Switch checked={settings.quietHoursEnabled} onChange={value=>update("quietHoursEnabled",value)} label={t("quietHours")}/>{settings.quietHoursEnabled&&<div className="form-grid inset-fields"><Field label={t("quietStart")}><input type="time" value={settings.quietStart} onChange={event=>update("quietStart",event.target.value)}/></Field><Field label={t("quietEnd")}><input type="time" value={settings.quietEnd} onChange={event=>update("quietEnd",event.target.value)}/></Field><Field label={t("quietAction")}><select value={settings.quietAction} onChange={event=>update("quietAction",event.target.value)}><option value="silent">{t("silent")}</option><option value="pause">{t("pause")}</option><option value="importantOnly">{t("importantOnly")}</option></select></Field></div>}</Card><Card title={t("contentSafety")}><div className="setting-list"><Switch checked={settings.permissionNotifications} onChange={value=>update("permissionNotifications",value)} label={t("permissionAlerts")}/><Switch checked={settings.redactSensitive} onChange={value=>update("redactSensitive",value)} label={t("privacy")}/></div><p className="card-note">{t("contentSafetyHint")}</p></Card></div></div>;
}

function SystemPage({settings,update,state,onRefresh,onError,onNotice}:{settings:AppSettings;update:<K extends keyof AppSettings>(key:K,value:AppSettings[K])=>void;state:AppState;onRefresh:()=>Promise<void>;onError:(value:string)=>void;onNotice:(value:string)=>void}){
  const{t,i18n}=useTranslation();const[diag,setDiag]=useState<Diagnostics|null>(null);const[autostart,setAutostart]=useState(false);const[busy,setBusy]=useState<"install"|"uninstall"|"check"|null>(null);
  useEffect(()=>{void api.autostart().then(setAutostart).catch(()=>{})},[]);
  const install=async()=>{setBusy("install");try{const hook=await api.installHook();await onRefresh();onNotice(t(hook.trusted&&hook.enabled?"hookTrusted":"hookInstalled"))}catch(value){onError(message(value,t("unknownError")))}finally{setBusy(null)}};
  const uninstall=async()=>{setBusy("uninstall");try{await api.uninstallHook();await onRefresh();onNotice(t("hookUninstalled"))}catch(value){onError(message(value,t("unknownError")))}finally{setBusy(null)}};
  const checkTrust=async()=>{setBusy("check");try{const hook=await api.hookStatus();await onRefresh();onNotice(t(hook.trusted&&hook.enabled?"hookTrusted":"hookNeedsTrust"))}catch(value){onError(message(value,t("unknownError")))}finally{setBusy(null)}};
  const setStartup=async(value:boolean)=>{setAutostart(value);try{await api.setAutostart(value)}catch(error){setAutostart(!value);onError(message(error,t("unknownError")))}};
  const trustLabel=state.hook.trustStatus==="trusted"?t("trusted"):state.hook.trustStatus==="modified"?t("modified"):state.hook.trustStatus==="untrusted"?t("untrusted"):t("unknownTrust");
  return <div className="content-grid"><div className="stack"><Card title={t("hookManagement")}><div className="hook-summary"><StatusItem label={t("installedStatus")} value={state.hook.installed?t("online"):t("offline")} ok={state.hook.installed}/><StatusItem label={t("enabledStatus")} value={state.hook.enabled?t("enabledStatus"):t("disabledStatus")} ok={state.hook.enabled}/><StatusItem label={t("trustStatus")} value={trustLabel} ok={state.hook.trusted}/></div><code className="path-code">{state.hook.hooksPath}</code>{state.hook.reviewRequired&&<div className="hook-review"><strong>{t("hookNeedsTrust")}</strong><p>{t("hookTrustHelp")}</p><code>/hooks</code></div>}<div className="button-row hook-actions"><Button kind="primary" disabled={busy!==null} onClick={()=>void install()}>{busy==="install"?t("installing"):t("installRepair")}</Button><Button disabled={busy!==null||!state.hook.installed} onClick={()=>void checkTrust()}>{busy==="check"?t("checkingTrust"):t("checkTrust")}</Button><Button kind="danger" disabled={busy!==null||!state.hook.installed} onClick={()=>void uninstall()}>{busy==="uninstall"?t("uninstalling"):t("uninstall")}</Button></div></Card>
    <Card title={t("diagnostics")} action={<Button onClick={()=>void api.diagnostics().then(setDiag).catch(value=>onError(message(value,t("unknownError"))))}>{t("runDiagnostics")}</Button>}>{diag?<div className="diagnostics"><Diagnostic label="Settings" ok={diag.settingsReadable}/><Diagnostic label="SQLite" ok={diag.databaseReady}/><Diagnostic label="Credential store" ok={diag.credentialStoreAvailable}/><Diagnostic label={t("binary")} ok={diag.hookBinaryExists}/><Diagnostic label={t("trustStatus")} ok={diag.hook.trusted}/><code>{diag.hookBinary}</code></div>:<div className="empty-state compact"><strong>{t("diagnosticsIdle")}</strong><span>{t("diagnosticsHint")}</span></div>}</Card></div>
    <div className="stack"><Card title={t("appearance")}><div className="form-grid"><Field label={t("language")}><select value={settings.language} onChange={event=>{update("language",event.target.value);void i18n.changeLanguage(event.target.value==="system"?(navigator.language.startsWith("zh")?"zh":"en"):event.target.value)}}><option value="system">{t("systemDefault")}</option><option value="zh">简体中文</option><option value="en">English</option></select></Field><Field label={t("theme")}><select value={settings.theme} onChange={event=>update("theme",event.target.value)}><option value="system">{t("systemDefault")}</option><option value="light">{t("light")}</option><option value="dark">{t("dark")}</option></select></Field></div><div className="setting-list"><Switch checked={autostart} onChange={value=>void setStartup(value)} label={t("autostart")}/></div></Card><Card title={t("resourceProfile")}><div className="resource-card"><span className="resource-icon"><RocketIcon/></span><div><strong>{t("adaptiveIdle")}</strong><p>{t("adaptiveIdleHint")}</p></div></div></Card></div></div>;
}

function Diagnostic({label,ok}:{label:string;ok:boolean}){return <div><span>{label}</span><Badge tone={ok?"sent":"failed"}>{ok?"OK":"FAIL"}</Badge></div>}

function Setup({settings,update,finish,onError}:{settings:AppSettings;update:<K extends keyof AppSettings>(key:K,value:AppSettings[K])=>void;finish:(key:string)=>Promise<void>;onError:(value:string)=>void}){const{t}=useTranslation();const[key,setKey]=useState("");const[busy,setBusy]=useState(false);const complete=async()=>{setBusy(true);try{await finish(key)}catch(value){onError(message(value,t("unknownError")))}finally{setBusy(false)}};return <div className="modal-backdrop"><div className="setup-modal"><BrandMark className="setup-logo"/><span className="kicker">CODEXNOTIFY</span><h2>{t("setupTitle")}</h2><p>{t("setupBody")}</p><div className="stack"><Field label={t("barkServer")}><input value={settings.barkServer} onChange={event=>update("barkServer",event.target.value)}/></Field><Field label={t("deviceKey")}><input type="password" value={key} onChange={event=>setKey(event.target.value)}/></Field></div><div className="button-row"><Button kind="primary" disabled={!key||busy} onClick={()=>void complete()}>{busy?t("installing"):t("completeSetup")}</Button></div></div></div>}

function message(value:unknown,fallback:string){const error=value as Partial<ApiError>|undefined;if(error?.code&&i18n.exists(`errors.${error.code}`))return i18n.t(`errors.${error.code}`);return error?.message||fallback}
