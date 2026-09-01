import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import "./i18n";
import { App } from "./App";

const {settings,hook,installHook}=vi.hoisted(()=>{
  const settings={schemaVersion:1,enabled:true,barkServer:"https://api.day.app",group:"Codex",level:"active",sound:"",scope:"all",projects:[],messageMode:"summary200",fixedMessage:"done",notificationTitle:"{project}",permissionNotifications:true,redactSensitive:true,quietHoursEnabled:false,quietStart:"22:00",quietEnd:"08:00",quietAction:"silent",barkIcon:"",clickUrl:"",requestTimeout:8,retryLimit:5,encryptionEnabled:false,encryptionAlgorithm:"AES-128-CBC",setupCompleted:true,language:"en",theme:"light"};
  const hook={hooksPath:"/tmp/hooks.json",exists:true,installed:true,handlerCount:2,installedEvents:["Stop","PermissionRequest"],pathCurrent:true,configuredCommand:"hook",trusted:true,trustStatus:"trusted",reviewRequired:false,enabled:true};
  return{settings,hook,installHook:vi.fn().mockResolvedValue({...hook,trusted:false,trustStatus:"untrusted",reviewRequired:true})};
});
vi.mock("./api",()=>({api:{state:vi.fn().mockResolvedValue({settings,counts:{queued:0,sending:0,retrying:0,sent:4,failed:0,suppressed:0},secrets:{barkKeyConfigured:true,encryptionKeyConfigured:false},hook,health:{}}),events:vi.fn().mockResolvedValue([]),autostart:vi.fn().mockResolvedValue(false),installHook,uninstallHook:vi.fn().mockResolvedValue(hook),hookStatus:vi.fn().mockResolvedValue(hook)}}));

afterEach(()=>cleanup());

test("renders a healthy overview",async()=>{render(<App/>);await waitFor(()=>expect(screen.getByText("Delivery ready")).toBeInTheDocument());expect(screen.getAllByText("Ready").length).toBeGreaterThan(0);});

test("install repair shows visible completion feedback",async()=>{render(<App/>);await waitFor(()=>expect(screen.getByText("System")).toBeInTheDocument());fireEvent.click(screen.getByText("System"));fireEvent.click(await screen.findByText("Install / repair"));await waitFor(()=>expect(installHook).toHaveBeenCalled());expect(await screen.findByText("Hook installed/repaired. Review its trust status below.")).toBeInTheDocument();});
