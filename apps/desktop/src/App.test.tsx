import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import "./i18n";
import { App, StorageMigration, StorageSetup } from "./App";

vi.mock("@tauri-apps/plugin-dialog",()=>({open:vi.fn()}));

const {settings,hook,installHook,selectStorage,migrateStorage,testBark,testHook,setSecret}=vi.hoisted(()=>{
  const settings={schemaVersion:1,enabled:true,barkServer:"https://api.day.app",group:"Codex",level:"active",sound:"",scope:"all",projects:[],messageMode:"summary200",fixedMessage:"done",notificationTitle:"{project}",permissionNotifications:true,redactSensitive:true,quietHoursEnabled:false,quietStart:"22:00",quietEnd:"08:00",quietAction:"silent",barkIcon:"",clickUrl:"",requestTimeout:8,retryLimit:5,encryptionEnabled:false,encryptionAlgorithm:"AES-128-CBC",setupCompleted:true,language:"en",theme:"light"};
  const hook={hooksPath:"/tmp/hooks.json",exists:true,installed:true,handlerCount:2,installedEvents:["Stop","PermissionRequest"],pathCurrent:true,configuredCommand:"hook",trusted:true,trustStatus:"trusted",reviewRequired:false,enabled:true};
  return{settings,hook,installHook:vi.fn().mockResolvedValue({...hook,trusted:false,trustStatus:"untrusted",reviewRequired:true}),selectStorage:vi.fn().mockResolvedValue(undefined),migrateStorage:vi.fn().mockResolvedValue(undefined),testBark:vi.fn().mockResolvedValue({ok:true,elapsedMs:42}),testHook:vi.fn().mockResolvedValue({ok:true,elapsedMs:55,deliveryStatus:"sent",errorCode:"",message:""}),setSecret:vi.fn().mockResolvedValue(undefined)};
});
vi.mock("./api",()=>({api:{state:vi.fn().mockResolvedValue({storage:{configured:true,mode:"default",root:"/tmp/CodexNotify",configDir:"/tmp/config",dataDir:"/tmp/data",logDir:"/tmp/logs",locatorFile:"/tmp/storage.json"},settings,counts:{queued:0,sending:0,retrying:0,sent:4,failed:0,suppressed:0},secrets:{barkKeyConfigured:true,encryptionKeyConfigured:false},hook,health:{status:"success",deliveryStatus:"sent",lastAttemptAt:"2026-09-02T00:00:00Z"}}),events:vi.fn().mockResolvedValue([]),autostart:vi.fn().mockResolvedValue(false),installHook,selectStorage,migrateStorage,testBark,testHook,setSecret,save:vi.fn().mockResolvedValue(undefined),deleteSecret:vi.fn().mockResolvedValue(undefined),uninstallHook:vi.fn().mockResolvedValue(hook),hookStatus:vi.fn().mockResolvedValue(hook),diagnostics:vi.fn()}}));

afterEach(()=>{cleanup();vi.clearAllMocks();testBark.mockResolvedValue({ok:true,elapsedMs:42})});

test("renders a healthy overview",async()=>{render(<App/>);await waitFor(()=>expect(screen.getByText("Delivery ready")).toBeInTheDocument());expect(screen.getAllByText("Ready").length).toBeGreaterThan(0);});

test("full-chain test invokes the standalone Hook",async()=>{render(<App/>);fireEvent.click(await screen.findByText("Test full chain"));await waitFor(()=>expect(testHook).toHaveBeenCalledTimes(1));expect(await screen.findByText("Hook and Bark delivery verified in 55 ms.")).toBeInTheDocument();});

test("install repair shows visible completion feedback",async()=>{render(<App/>);await waitFor(()=>expect(screen.getByText("System")).toBeInTheDocument());fireEvent.click(screen.getByText("System"));fireEvent.click(await screen.findByText("Install / repair"));await waitFor(()=>expect(installHook).toHaveBeenCalled());expect(await screen.findByText("Hook installed/repaired. Review its trust status below.")).toBeInTheDocument();});

test("first launch selects the default data location",async()=>{render(<StorageSetup onError={vi.fn()}/>);fireEvent.click(screen.getByText("Use this location"));await waitFor(()=>expect(selectStorage).toHaveBeenCalledWith("default",""));});

test("storage migration warns before moving and deleting the old location",async()=>{render(<StorageMigration current={{configured:true,mode:"default",root:"/old",configDir:"/old/config",dataDir:"/old/data",logDir:"/old/logs",locatorFile:"/system/storage.json"}} close={vi.fn()} onError={vi.fn()}/>);expect(screen.getByText("The previous location will be removed")).toBeInTheDocument();fireEvent.click(screen.getByText("Move and restart"));await waitFor(()=>expect(migrateStorage).toHaveBeenCalledWith("portable",""));});

test("invalid Bark key stays editable and is not persisted",async()=>{testBark.mockRejectedValueOnce({code:"barkInvalidKey",message:"secret response"});render(<App/>);fireEvent.click(await screen.findByText("Push"));const input=await screen.findByLabelText("Device key");fireEvent.change(input,{target:{value:"wrong-key"}});fireEvent.click(screen.getByText("Save & test"));expect(await screen.findByText("This Bark device key is invalid. Check it and try again.")).toBeInTheDocument();expect(input).toHaveValue("wrong-key");expect(setSecret).not.toHaveBeenCalled();});

test("valid Bark key is persisted only after the test succeeds",async()=>{render(<App/>);fireEvent.click(await screen.findByText("Push"));fireEvent.change(await screen.findByLabelText("Device key"),{target:{value:"valid-key"}});fireEvent.click(screen.getByText("Save & test"));await waitFor(()=>expect(testBark).toHaveBeenCalledWith(expect.objectContaining({barkKey:"valid-key"})));await waitFor(()=>expect(setSecret).toHaveBeenCalledWith("barkKey","valid-key"));expect(await screen.findByText("Test delivered in 42 ms.")).toBeInTheDocument();});
