import * as SwitchPrimitive from "@radix-ui/react-switch";
import type { PropsWithChildren, ReactNode } from "react";

export function Card({title,action,children,className=""}:{title?:string;action?:ReactNode;children:ReactNode;className?:string}){return <section className={`card ${className}`}><div className="card-head">{title&&<h2>{title}</h2>}{action}</div>{children}</section>}
export function Switch({checked,onChange,label}:{checked:boolean;onChange:(v:boolean)=>void;label:string}){return <label className="switch-row"><span>{label}</span><SwitchPrimitive.Root className="switch" checked={checked} onCheckedChange={onChange}><SwitchPrimitive.Thumb className="switch-thumb"/></SwitchPrimitive.Root></label>}
export function Field({label,children,hint}:{label:string;children:ReactNode;hint?:string}){return <label className="field"><span>{label}</span>{children}{hint&&<small>{hint}</small>}</label>}
export function Button({kind="secondary",className="",...props}:PropsWithChildren<React.ButtonHTMLAttributes<HTMLButtonElement>&{kind?:"primary"|"secondary"|"danger"}>){return <button className={`button ${kind} ${className}`} {...props}/>}
export function Badge({tone="neutral",children}:{tone?:string;children:ReactNode}){return <span className={`badge ${tone}`}>{children}</span>}

export function BrandMark({className=""}:{className?:string}){return <svg className={className} viewBox="0 0 64 64" role="img" aria-label="CodexNotify"><defs><linearGradient id="brand-gradient" x1="8" y1="6" x2="57" y2="60" gradientUnits="userSpaceOnUse"><stop stopColor="#7c5cff"/><stop offset="1" stopColor="#3b82f6"/></linearGradient></defs><rect width="64" height="64" rx="16" fill="url(#brand-gradient)"/><path d="M18 45V19l28 26V19" fill="none" stroke="white" strokeWidth="7" strokeLinecap="round" strokeLinejoin="round"/><circle cx="50" cy="13" r="5" fill="#8ff0c7" stroke="#fff" strokeWidth="2"/></svg>}
