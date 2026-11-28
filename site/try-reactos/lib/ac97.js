// The virtual AC'97 sound card for the ReactOS demo, built from site/try-reactos/packages/ac97 by `bun run build`. Do not edit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
var F=256,V=64,m={DCH:1,CELV:2,LVBCI:4,BCIS:8,FIFOE:16},b={RPBM:1,RR:2,LVBIE:4,FEIE:8,IOCE:16},y={CODEC_READY:256,PIINT:32,POINT:64,MINT:128},M=[0,16,32];var k=()=>({l:new Float32Array(0),r:new Float32Array(0),frac:0});function N(){return{bdbar:0,civ:0,lvi:0,sr:m.DCH,picb:0,piv:0,cr:0,addr:0,len:0,pos:0,loaded:!1,haltedAtLast:!1,ioc:!1,bup:!1}}function G(){return{dmaFrames:0,peakRaw:0,buffersDone:0,lastValidHalts:0,iocs:0,starvedFrames:0,irqRaised:0,bdlReads:0}}function P(e){if(typeof e!=="object"||e===null)return!1;let r=e;if(r.version!==1)return!1;if(!Array.isArray(r.nam)||r.nam.length!==128||!r.nam.every((t)=>Number.isInteger(t)&&t>=0&&t<=65535))return!1;if(!Number.isInteger(r.globCnt))return!1;if(!Array.isArray(r.ch)||r.ch.length!==3)return!1;return r.ch.every((t)=>typeof t==="object"&&t!==null&&Number.isInteger(t.bdbar)&&Number.isInteger(t.civ)&&Number.isInteger(t.lvi)&&Number.isInteger(t.sr)&&Number.isInteger(t.cr))}class O{readMem;raiseIrq;lowerIrq;trace;nam=new Uint16Array(128);ch=[];globCnt=0;irqAsserted=!1;carry=k();counters=G();constructor({readMem:e,raiseIrq:r=()=>{},lowerIrq:t=()=>{},trace:n=null}){this.readMem=e,this.raiseIrq=r,this.lowerIrq=t,this.trace=n,this.reset()}reset(){this.resetCodec(),this.ch=Array.from({length:3},N),this.globCnt=0,this.irqAsserted=!1,this.carry=k(),this.counters=G()}word(e){return this.nam[e]??0}channel(e){let r=this.ch[e];if(!r)throw RangeError("no channel "+e);return r}resetCodec(){let e=this.nam;e.fill(0),e[0]=320,e[1]=32768,e[2]=32768,e[3]=32768,e[5]=0,e[6]=32776,e[7]=32776,e[8]=34824,e[9]=34824,e[10]=34824,e[11]=34824,e[12]=34824,e[13]=0,e[14]=32768,e[15]=32768,e[16]=0,e[20]=1,e[21]=0,e[22]=48000,e[25]=48000,e[62]=33668,e[63]=30208}get vra(){return(this.word(21)&1)===1}get rate(){return this.vra?this.word(22)||48000:48000}gains(){let e=(n,i,s)=>{let l=this.word(n>>1);if(l&32768)return 0;let u=l>>i&31;return Math.pow(10,-(u-s)*1.5/20)},r=(n)=>e(2,n,0),t=(n)=>e(24,n,8);return{l:Math.min(1,r(8)*t(8)),r:Math.min(1,r(0)*t(0))}}namRead(e,r){if(r===1){let n=this.namRead(e&-2,2);return(e&1?n>>8:n)&255}if(r===4)return(this.namRead(e,2)|this.namRead(e+2,2)<<16)>>>0;e&=254;let t=this.word(e>>1);if(e===38)t=15|t&65280;if(e===42)t=t&1|128;return this.log("nam-r",e,t),t}namWrite(e,r,t){if(r===1){let n=this.word((e&-2)>>1);this.namWrite(e&-2,2,e&1?n&255|(t&255)<<8:n&65280|t&255);return}if(r===4){this.namWrite(e,2,t&65535),this.namWrite(e+2,2,t>>>16);return}switch(e&=254,t&=65535,this.log("nam-w",e,t),e){case 0:this.resetCodec();return;case 40:case 124:case 126:return;case 42:this.nam[e>>1]=t&1;return;case 44:case 50:if(this.vra)this.nam[e>>1]=t;return;default:this.nam[e>>1]=t}}nabmRead(e,r){let t=0;for(let n=0;n<r;n++)t|=this.nabmRead8(e+n)<<8*n;return this.log("nabm-r"+r,e,t>>>0),t>>>0}nabmWrite(e,r,t){this.log("nabm-w"+r,e,t>>>0);for(let n=0;n<r;n++)this.nabmWrite8(e+n,t>>>8*n&255);this.updateIrq()}channelAt(e){return M.findIndex((r)=>e>=r&&e<r+12)}nabmRead8(e){if(e>=44&&e<48)return this.globCnt>>>8*(e-44)&255;if(e>=48&&e<52)return this.globSta()>>>8*(e-48)&255;if(e===52)return 0;let r=this.channelAt(e);if(r<0)return 0;let t=this.channel(r),n=e-(M[r]??0);switch(n){case 0:case 1:case 2:case 3:return t.bdbar>>>8*n&255;case 4:return t.civ;case 5:return t.lvi;case 6:return t.sr&255;case 7:return 0;case 8:return t.picb&255;case 9:return t.picb>>8&255;case 10:return t.piv;case 11:return t.cr}return 0}nabmWrite8(e,r){if(e>=44&&e<48){let s=8*(e-44);this.globCnt=(this.globCnt&~(255<<s)|r<<s)>>>0;return}if(e>=48&&e<52)return;let t=this.channelAt(e);if(t<0)return;let n=this.channel(t),i=e-(M[t]??0);switch(i){case 0:case 1:case 2:case 3:{let s=8*i;n.bdbar=((n.bdbar&~(255<<s)|r<<s)&-8)>>>0;break}case 5:if(n.lvi=r&31,t===1&&n.haltedAtLast&&n.cr&b.RPBM)n.haltedAtLast=!1,n.sr&=~(m.DCH|m.CELV),n.civ=n.civ+1&31,n.loaded=!1;break;case 6:n.sr&=~(r&(m.LVBCI|m.BCIS|m.FIFOE));break;case 11:this.writeCr(t,r);break}}writeCr(e,r){let t=this.channel(e);if(r&b.RR){if(this.ch[e]=N(),e===1)this.carry=k();return}let n=(t.cr&b.RPBM)!==0;if(t.cr=r&(b.RPBM|b.LVBIE|b.FEIE|b.IOCE),t.cr&b.RPBM&&!n)t.sr&=~m.DCH,t.haltedAtLast=!1,t.loaded=!1;else if(!(t.cr&b.RPBM)&&n)t.sr|=m.DCH}globSta(){let e=y.CODEC_READY,r=(t)=>Boolean(t.sr&m.BCIS&&t.cr&b.IOCE)||Boolean(t.sr&m.LVBCI&&t.cr&b.LVBIE)||Boolean(t.sr&m.FIFOE&&t.cr&b.FEIE);if(r(this.channel(0)))e|=y.PIINT;if(r(this.channel(1)))e|=y.POINT;if(r(this.channel(2)))e|=y.MINT;return e}irqPending(){return(this.globSta()&(y.PIINT|y.POINT|y.MINT))!==0}updateIrq(){let e=this.irqPending();if(e&&!this.irqAsserted)this.irqAsserted=!0,this.counters.irqRaised++,this.raiseIrq();else if(!e&&this.irqAsserted)this.irqAsserted=!1,this.lowerIrq()}get interruptAsserted(){return this.irqAsserted}releaseIrq(){if(this.irqAsserted)this.irqAsserted=!1,this.lowerIrq()}get running(){let e=this.channel(1);return(e.cr&b.RPBM)!==0&&(e.sr&m.DCH)===0}loadBuffer(e){let r=this.readMem(e.bdbar+e.civ*8>>>0,8),t=new DataView(r.buffer,r.byteOffset,8);this.counters.bdlReads++,e.addr=(t.getUint32(0,!0)&-2)>>>0;let n=t.getUint32(4,!0);if(e.len=n&65535,e.ioc=(n&2147483648)!==0,e.bup=(n&1073741824)!==0,e.pos=0,e.picb=e.len,e.piv=e.civ+1&31,e.loaded=!0,e.civ===e.lvi)e.sr|=m.CELV;else e.sr&=~m.CELV}finishBuffer(e){if(this.counters.buffersDone++,e.ioc)e.sr|=m.BCIS,this.counters.iocs++;if(e.civ===e.lvi)e.sr|=m.LVBCI|m.DCH|m.CELV,this.counters.lastValidHalts++,e.haltedAtLast=!0,e.loaded=!1;else e.civ=e.civ+1&31,e.loaded=!1}readFrames(e,r,t,n=0,i=!1){let s=this.channel(1),l=0,u=64;while(l<e&&this.running&&u-- >0){if(!s.loaded){if(i&&s.sr&m.BCIS&&s.cr&b.IOCE)break;this.loadBuffer(s)}let o=Math.min(s.len-s.pos>>1,e-l);if(o>0){let c=this.readMem(s.addr+s.pos*2>>>0,o*4),p=new Int16Array(c.buffer.slice(c.byteOffset,c.byteOffset+o*4));for(let f=0;f<o;f++){let a=(p[2*f]??0)/32768,d=(p[2*f+1]??0)/32768;r[n+l+f]=a,t[n+l+f]=d;let x=Math.max(Math.abs(a),Math.abs(d));if(x>this.counters.peakRaw)this.counters.peakRaw=x}s.pos+=o*2,s.picb=s.len-s.pos,l+=o,this.counters.dmaFrames+=o}if(s.len-s.pos<2)s.picb=0,this.finishBuffer(s)}return this.updateIrq(),l}render(e,r,t,n,i=!1){let s=this.carry,l=this.rate/r,u=this.gains(),o=0,c=Math.ceil(s.frac+(e-1)*l)+2;if(s.l.length<c&&this.running){let a=c-s.l.length,d=new Float32Array(a),x=new Float32Array(a),v=this.readFrames(a,d,x,0,i);if(v>0){let w=new Float32Array(s.l.length+v),S=new Float32Array(s.l.length+v);w.set(s.l),S.set(s.r),w.set(d.subarray(0,v),s.l.length),S.set(x.subarray(0,v),s.l.length),s.l=w,s.r=S}}let p=s.frac;for(let a=0;a<e;a++){let d=Math.floor(p);if(d+1<s.l.length){let x=p-d;t[a]=((s.l[d]??0)*(1-x)+(s.l[d+1]??0)*x)*u.l,n[a]=((s.r[d]??0)*(1-x)+(s.r[d+1]??0)*x)*u.r,o++,p+=l}else if(t[a]=0,n[a]=0,this.running&&!i)this.counters.starvedFrames++}let f=Math.min(Math.floor(p),s.l.length);return s.l=s.l.slice(f),s.r=s.r.slice(f),s.frac=p-f,o}log(e,r,t){if(this.trace)this.trace(e,r,t)}getState(){return{version:1,nam:Array.from(this.nam),globCnt:this.globCnt,ch:this.ch.map((e)=>({...e}))}}setState(e){if(!P(e))throw TypeError("not a saved AC'97 state");this.nam=Uint16Array.from(e.nam),this.globCnt=e.globCnt,this.ch=e.ch.map((r)=>({...N(),...r})),this.carry=k(),this.irqAsserted=!1,this.updateIrq()}}var ue=`
class RingPlayer extends AudioWorkletProcessor {
	constructor() {
		super();
		this.cap = 96000;
		this.l = new Float32Array(this.cap);
		this.r = new Float32Array(this.cap);
		this.rd = 0;
		this.wr = 0;
		this.n = 0;
		this.popped = 0;
		this.primed = false;
		this.starving = false;
		this.s = { out: 0, silent: 0, peak: 0, underrunEvents: 0, underrunFrames: 0, dropped: 0, quanta: 0 };
		this.rec = null;
		this.tick = 0;
		this.port.onmessage = (e) => {
			const m = e.data;
			if (m.t === "push") {
				const len = m.l.length;
				for (let i = 0; i < len; i++) {
					if (this.n >= this.cap) { this.s.dropped += len - i; break; }
					this.l[this.wr] = m.l[i];
					this.r[this.wr] = m.r[i];
					this.wr = (this.wr + 1) % this.cap;
					this.n++;
				}
				this.primed = true;
			} else if (m.t === "clear") {
				this.rd = 0;
				this.wr = 0;
				this.n = 0;
				this.primed = false;
				this.starving = false;
				this.port.postMessage({ t: "cleared", popped: this.popped });
			} else if (m.t === "rec") {
				this.rec = m.on ? { l: [], r: [], frames: 0, max: m.max || 48000 * 60 } : null;
			} else if (m.t === "take") {
				const r = this.rec;
				this.rec = null;
				this.port.postMessage({ t: "rec", l: r ? r.l : [], r: r ? r.r : [] });
			} else if (m.t === "stats") {
				this.port.postMessage({ t: "stats", s: this.s, n: this.n });
			}
		};
	}
	process(inputs, outputs) {
		const o = outputs[0];
		const len = o[0].length;
		const ol = o[0];
		const or = o[1] || o[0];
		const have = Math.min(this.n, len);
		for (let i = 0; i < have; i++) {
			ol[i] = this.l[this.rd];
			or[i] = this.r[this.rd];
			this.rd = (this.rd + 1) % this.cap;
		}
		this.n -= have;
		this.popped += have;
		for (let i = have; i < len; i++) { ol[i] = 0; or[i] = 0; }
		if (this.primed && have < len) {
			if (!this.starving) this.s.underrunEvents++;
			this.starving = true;
			this.s.underrunFrames += len - have;
		} else if (have === len) {
			this.starving = false;
		}
		const st = this.s;
		st.quanta++;
		for (let i = 0; i < len; i++) {
			const a = Math.max(Math.abs(ol[i]), Math.abs(or[i]));
			if (a > st.peak) st.peak = a;
			if (a < 0.0001) st.silent++;
		}
		st.out += len;
		if (this.rec && this.rec.frames < this.rec.max) {
			this.rec.l.push(ol.slice(0));
			this.rec.r.push(or.slice(0));
			this.rec.frames += len;
		}
		if (++this.tick % 4 === 0) this.port.postMessage({ t: "level", popped: this.popped });
		return true;
	}
}
registerProcessor("ac97-ring-player", RingPlayer);
`,_=new WeakMap;function ne(e){let r=_.get(e);if(!r){let t=URL.createObjectURL(new Blob([`
class RingPlayer extends AudioWorkletProcessor {
	constructor() {
		super();
		this.cap = 96000;
		this.l = new Float32Array(this.cap);
		this.r = new Float32Array(this.cap);
		this.rd = 0;
		this.wr = 0;
		this.n = 0;
		this.popped = 0;
		this.primed = false;
		this.starving = false;
		this.s = { out: 0, silent: 0, peak: 0, underrunEvents: 0, underrunFrames: 0, dropped: 0, quanta: 0 };
		this.rec = null;
		this.tick = 0;
		this.port.onmessage = (e) => {
			const m = e.data;
			if (m.t === "push") {
				const len = m.l.length;
				for (let i = 0; i < len; i++) {
					if (this.n >= this.cap) { this.s.dropped += len - i; break; }
					this.l[this.wr] = m.l[i];
					this.r[this.wr] = m.r[i];
					this.wr = (this.wr + 1) % this.cap;
					this.n++;
				}
				this.primed = true;
			} else if (m.t === "clear") {
				this.rd = 0;
				this.wr = 0;
				this.n = 0;
				this.primed = false;
				this.starving = false;
				this.port.postMessage({ t: "cleared", popped: this.popped });
			} else if (m.t === "rec") {
				this.rec = m.on ? { l: [], r: [], frames: 0, max: m.max || 48000 * 60 } : null;
			} else if (m.t === "take") {
				const r = this.rec;
				this.rec = null;
				this.port.postMessage({ t: "rec", l: r ? r.l : [], r: r ? r.r : [] });
			} else if (m.t === "stats") {
				this.port.postMessage({ t: "stats", s: this.s, n: this.n });
			}
		};
	}
	process(inputs, outputs) {
		const o = outputs[0];
		const len = o[0].length;
		const ol = o[0];
		const or = o[1] || o[0];
		const have = Math.min(this.n, len);
		for (let i = 0; i < have; i++) {
			ol[i] = this.l[this.rd];
			or[i] = this.r[this.rd];
			this.rd = (this.rd + 1) % this.cap;
		}
		this.n -= have;
		this.popped += have;
		for (let i = have; i < len; i++) { ol[i] = 0; or[i] = 0; }
		if (this.primed && have < len) {
			if (!this.starving) this.s.underrunEvents++;
			this.starving = true;
			this.s.underrunFrames += len - have;
		} else if (have === len) {
			this.starving = false;
		}
		const st = this.s;
		st.quanta++;
		for (let i = 0; i < len; i++) {
			const a = Math.max(Math.abs(ol[i]), Math.abs(or[i]));
			if (a > st.peak) st.peak = a;
			if (a < 0.0001) st.silent++;
		}
		st.out += len;
		if (this.rec && this.rec.frames < this.rec.max) {
			this.rec.l.push(ol.slice(0));
			this.rec.r.push(or.slice(0));
			this.rec.frames += len;
		}
		if (++this.tick % 4 === 0) this.port.postMessage({ t: "level", popped: this.popped });
		return true;
	}
}
registerProcessor("ac97-ring-player", RingPlayer);
`],{type:"text/javascript"}));r=e.audioWorklet.addModule(t).finally(()=>URL.revokeObjectURL(t)),_.set(e,r),r.catch(()=>_.delete(e))}return r}function U(e,r,t){let n=r.currentTime,i=e.gain;i.cancelScheduledValues(n),i.setTargetAtTime(t,n,0.01),i.setValueAtTime(t,n+0.1)}var se=120,j=100;class Z{context;node;kind="worklet";output;rate;onNeed=null;pushed=0;popped=0;clearing=!1;lastAsk=Number.NEGATIVE_INFINITY;target;wanted=1;pending=[];constructor(e,r){this.context=e;this.node=r;this.rate=e.sampleRate,this.target=Math.round(this.rate*se/1000),this.output=e.createGain(),r.connect(this.output).connect(e.destination),r.port.onmessage=(t)=>{let n=t.data;if(n.t==="level")this.popped=n.popped??this.popped,this.lastAsk=performance.now(),this.top();else if(n.t==="cleared")this.popped=n.popped??this.popped,this.pushed=this.popped,this.clearing=!1;else if(n.t==="stats"||n.t==="rec")this.pending.splice(0).forEach((i)=>i(n))}}top(){if(this.clearing)return;let e=this.target-(this.pushed-this.popped);if(e<256||!this.onNeed)return;let r=this.onNeed(e);if(!r)return;this.pushed+=e,this.node.port.postMessage({t:"push",l:r.l,r:r.r},[r.l.buffer,r.r.buffer])}live(){return this.context.state==="running"&&performance.now()-this.lastAsk<j}setGain(e){this.wanted=e,this.ramp(e)}ramp(e){U(this.output,this.context,e)}gain(){return this.wanted}clear(){this.clearing=!0,this.node.port.postMessage({t:"clear"})}ask(e){return new Promise((r)=>{this.pending.push(r),this.node.port.postMessage(e)})}async stats(){return(await this.ask({t:"stats"})).s??null}record(e){this.node.port.postMessage({t:"rec",on:e})}async take(){let e=await this.ask({t:"take"});return{l:e.l??[],r:e.r??[]}}close(){this.node.port.onmessage=null;try{this.node.disconnect(),this.output.disconnect()}catch{}}}class Y{context;kind="script";output;rate;onNeed=null;lastAsk=Number.NEGATIVE_INFINITY;wanted=1;blocks=0;frames=0;node;constructor(e){this.context=e;this.rate=e.sampleRate,this.output=e.createGain(),this.node=e.createScriptProcessor(2048,0,2),this.node.onaudioprocess=(r)=>{let t=r.outputBuffer.getChannelData(0),n=r.outputBuffer.getChannelData(1);this.lastAsk=performance.now(),this.blocks++,this.frames+=t.length;let i=this.onNeed?this.onNeed(t.length):null;if(i)t.set(i.l),n.set(i.r);else t.fill(0),n.fill(0)},this.node.connect(this.output).connect(e.destination)}live(){return this.context.state==="running"&&performance.now()-this.lastAsk<j*2}setGain(e){this.wanted=e,this.ramp(e)}ramp(e){U(this.output,this.context,e)}gain(){return this.wanted}clear(){}async stats(){return{out:this.frames,silent:0,peak:0,underrunEvents:0,underrunFrames:0,dropped:0,quanta:this.blocks}}record(){}async take(){return{l:[],r:[]}}close(){this.node.onaudioprocess=null;try{this.node.disconnect(),this.output.disconnect()}catch{}}}async function K(e){if(e.audioWorklet&&typeof AudioWorkletNode==="function")try{await ne(e);let r=new AudioWorkletNode(e,"ac97-ring-player",{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2]});return new Z(e,r)}catch{}return new Y(e)}var ie="0.5.473",X=53248,J=54272;function Q(){let e=new Uint8Array(64),r=(t,n)=>{e[t]=n&255,e[t+1]=n>>8};return r(0,32902),r(2,9237),r(4,3),r(6,640),e[8]=2,e[10]=1,e[11]=4,r(16,X|1),r(20,J|1),r(44,32902),r(46,0),e[61]=1,e[62]=2,e[63]=24,Array.from(e)}function q(e,r=null){let t=e.devices.pci,n=3;while(n<32&&t.devices[n<<3])n++;if(n>=32)throw Error("no free PCI slot for the AC'97 device");let i=n<<3,s=!0,l=new O({readMem:(o,c)=>e.read_blob(o,c),raiseIrq:()=>s&&t.raise_irq(i),lowerIrq:()=>t.lower_irq(i),trace:r}),u=(o,c,p,f)=>{for(let a=0;a<c;a++)e.io.register_read(o+a,l,()=>s?p(a,1):255,()=>s?p(a,2):65535,()=>s?p(a,4):4294967295),e.io.register_write(o+a,l,(d)=>s&&f(a,1,d),(d)=>s&&f(a,2,d),(d)=>s&&f(a,4,d))};return u(X,F,(o,c)=>l.namRead(o,c),(o,c,p)=>l.namWrite(o,c,p)),u(J,V,(o,c)=>l.nabmRead(o,c),(o,c,p)=>l.nabmWrite(o,c,p)),t.register_device({pci_id:i,pci_space:Q(),pci_bars:[{size:F},{size:V}],name:"AC97"}),{model:l,pciId:i,retire(){l.releaseIrq(),s=!1}}}function B(e,r){let t=e.v86;if(t&&t.cpu&&t.cpu.devices&&t.cpu.devices.pci)return r(t.cpu),()=>{};let n=t,i=null,s=!1,l=(u)=>{let o=u.init;i=()=>{u.init=o,i=null},u.init=(...c)=>{let p=o.apply(u,c);if(i)i();if(!s)s=!0,r(u.cpu);return p}};if(Object.defineProperty(e,"v86",{configurable:!0,enumerable:!0,get:()=>n,set(u){if(n=u,u&&!s)l(u)}}),n)l(n);return()=>{if(s=!0,i)i();Object.defineProperty(e,"v86",{configurable:!0,enumerable:!0,writable:!0,value:n})}}var W=(e)=>Number.isFinite(e)?Math.min(1,Math.max(0,e)):0;function z(e,r){let t=W(e);return r?0:t*t}var ae=10,L=4800;function xe(e,r={}){let t=W(r.volume??0.7),n=Boolean(r.muted),i=null,s=null,l=!1,u=!1,o=!1,c="",p=0,f=performance.now(),a=null,d=()=>{},x=new Promise((h)=>d=h),v=()=>{if(!u&&r.onChange)r.onChange()},w=new Float32Array(L),S=new Float32Array(L),ee=setInterval(()=>{let h=performance.now();if(s&&s.live()){f=h;return}let g=h-f;f=h;let A=i?i.model:null;if(!A||!A.running)return;let I=s?s.rate:48000,C=Math.min(L,Math.round(g*I/1000));if(C<1)return;A.render(C,I,w,S,!0),p+=C},ae),E=()=>{if(s)s.setGain(z(t,n))},D=()=>v(),te=(h)=>{K(h).then((g)=>{if(u){g.close();return}s=g,s.onNeed=(A)=>{let I=i?i.model:null;if(!I)return null;let C=new Float32Array(A),H=new Float32Array(A);return I.render(A,g.rate,C,H),{l:C,r:H}},E(),v()},()=>{l=!0,v()})},T=typeof AudioContext==="function"?AudioContext:null;if(r.context)a=r.context;else if(r.context===void 0&&T)try{a=new T({sampleRate:48000,latencyHint:"interactive"}),o=!0}catch{l=!0}if(a)a.addEventListener("statechange",D),te(a);let re=B(e,(h)=>{if(u)return;if(i=q(h,r.trace??null),r.state)if(P(r.state))try{i.model.setState(r.state)}catch(g){c=g instanceof Error?g.message:String(g)}else c="the saved registers are not an AC'97 state";d(),v()}),R=()=>u?"none":l?"failed":a?a.state:"none";return{getAc97State:()=>i?i.model.getState():null,setVolume(h){t=W(h),E()},getVolume:()=>t,setMuted(h){n=h,E()},isMuted:()=>n,async resume(){if(a&&a.state!=="running"&&a.state!=="closed")try{await a.resume()}catch{}return R()},outputState:R,stats(){let h=i?i.model:null;return{registered:i!==null,pciId:i?i.pciId:null,guestRate:h?h.rate:0,playing:h?h.running:!1,interrupt:h?h.interruptAsserted:!1,counters:h?{...h.counters}:null,drainedFrames:p,output:R(),sink:s?s.kind:null,volume:t,muted:n,gain:z(t,n),stateError:c}},sinkStats:async()=>s?s.stats():null,get sink(){return s},get model(){return i?i.model:null},registered:x,detach(){if(u)return;if(u=!0,clearInterval(ee),re(),i)i.retire();if(s)s.clear(),s.close();if(a){if(a.removeEventListener("statechange",D),o)a.close().catch(()=>{})}s=null,d()}}}export{O as Ac97,b as CR,y as GS,V as NABM_SIZE,F as NAM_SIZE,m as SR,ie as V86_VERSION,ue as WORKLET_SOURCE,xe as attachAc97,K as createSink,z as gainFor,P as isAc97State,B as onceBeforeInit,Q as pciSpace,q as registerOn};
