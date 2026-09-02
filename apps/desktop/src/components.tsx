import * as SwitchPrimitive from "@radix-ui/react-switch";
import type { PropsWithChildren, ReactNode } from "react";
import brandIcon from "./assets/app-icon.png";

export function Card({title,action,children,className=""}:{title?:string;action?:ReactNode;children:ReactNode;className?:string}){return <section className={`card ${className}`}><div className="card-head">{title&&<h2>{title}</h2>}{action}</div>{children}</section>}
export function Switch({checked,onChange,label}:{checked:boolean;onChange:(v:boolean)=>void;label:string}){return <label className="switch-row"><span>{label}</span><SwitchPrimitive.Root className="switch" checked={checked} onCheckedChange={onChange}><SwitchPrimitive.Thumb className="switch-thumb"/></SwitchPrimitive.Root></label>}
export function Field({label,children,hint}:{label:string;children:ReactNode;hint?:string}){return <label className="field"><span>{label}</span>{children}{hint&&<small>{hint}</small>}</label>}
export function Button({kind="secondary",className="",...props}:PropsWithChildren<React.ButtonHTMLAttributes<HTMLButtonElement>&{kind?:"primary"|"secondary"|"danger"}>){return <button className={`button ${kind} ${className}`} {...props}/>}
export function Badge({tone="neutral",children}:{tone?:string;children:ReactNode}){return <span className={`badge ${tone}`}>{children}</span>}

export function BrandMark({className=""}:{className?:string}){return <img className={className} src={brandIcon} alt="CodexNotify"/>}
