// The virtual AC'97 sound card for the ReactOS demo, built from site/try-reactos/packages/ac97 by `bun run build`. Do not edit.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
var Le=[8000,11025,16000,22050,32000,44100,48000],We=8000,De=48000;function H(e){if(!Number.isFinite(e))return 48000;return Math.min(48000,Math.max(8000,Math.round(e)))}var Se=2048,le=new Map;function Ie(e,t){while(t)[e,t]=[t,e%t];return e}function ce(e){let t=1,r=1,n=e*e/4;for(let s=1;s<60;s++)if(r*=n/(s*s),t+=r,r<t*0.00000000000000001)break;return t}function Re(e){if(e>50)return 0.1102*(e-8.7);if(e>21)return 0.5842*Math.pow(e-21,0.4)+0.07886*(e-21);return 0}function Ee(e,t,r,n){let s=[e,t,r,n.taps,n.attenuationDb,n.passband].join(":"),i=le.get(s);if(i)return i;let a=Math.min(1,t/e),o=(n.attenuationDb-7.95)/(14.36*n.taps),u=Math.min(0.495,n.passband+o/2)*a,p=Math.ceil(n.taps/(2*a)),l=p*2,h=Re(n.attenuationDb),m=ce(h),x=new Float32Array(r*l),w=new Float64Array(l);for(let M=0;M<r;M++){let R=M/r,A=0;for(let C=0;C<l;C++){let _=C-p+1-R,b=_/p,P=Math.abs(b)<1?ce(h*Math.sqrt(1-b*b))/m:0,E=2*u*_,U=Math.abs(E)<0.000000000001?1:Math.sin(Math.PI*E)/(Math.PI*E),F=2*u*U*P;w[C]=F,A+=F}for(let C=0;C<l;C++)x[M*l+C]=(w[C]??0)/A}let I={phases:r,taps:l,coefs:x};return le.set(s,I),I}class O{inRate;outRate;phases;taps;lookahead;identity;half;coefs;step;buf;len;k;f=0;constructor(e,t,r={}){this.inRate=e,this.outRate=t,this.identity=e===t;let n={taps:r.taps??32,attenuationDb:r.attenuationDb??90,passband:r.passband??0.3},s=Ie(e,t),i=t/s;if(this.identity)this.phases=1,this.taps=1,this.half=0,this.lookahead=0,this.coefs=new Float32Array([1]),this.step=1;else{this.phases=Math.min(i,Se),this.step=this.phases===i?e/s:e/t*this.phases;let a=Ee(e,t,this.phases,n);this.taps=a.taps,this.half=a.taps/2,this.lookahead=this.half,this.coefs=a.coefs}this.buf=new Float32Array(8192),this.len=this.identity?0:this.half-1,this.k=this.len}reset(){this.len=this.identity?0:this.half-1,this.buf.fill(0,0,this.len),this.k=this.len,this.f=0}write(e,t,r,n=1,s=1){if(this.len+r>this.buf.length){let o=new Float32Array(Math.max(this.buf.length*2,this.len+r));o.set(this.buf.subarray(0,this.len)),this.buf=o}let i=this.buf,a=this.len;for(let o=0,c=t;o<r;o++,c+=n)i[a++]=(e[c]??0)*s;this.len=a}writeSilence(e){if(this.len+e>this.buf.length){let t=new Float32Array(Math.max(this.buf.length*2,this.len+e));t.set(this.buf.subarray(0,this.len)),this.buf=t}this.buf.fill(0,this.len,this.len+e),this.len+=e}ahead(e){let t=this.f+(e-1)*this.step,r=Math.floor(t/this.phases);if(this.step!==Math.floor(this.step)&&Math.floor(t-r*this.phases+0.5)>=this.phases)r++;return r}inputNeeded(e){if(e<=0)return 0;if(this.identity)return Math.max(0,e-(this.len-this.k));return Math.max(0,this.k+this.ahead(e)+this.half+1-this.len)}available(){if(this.identity)return this.len-this.k;let e=this.len-this.k-this.half-1;if(e<0)return 0;let t=Math.max(0,Math.floor((e*this.phases-this.f)/this.step)+1);while(this.inputNeeded(t+1)===0)t++;while(t>0&&this.inputNeeded(t)>0)t--;return t}read(e,t,r=0){let n=this.buf,s=0;if(this.identity){let i=Math.min(t,this.len-this.k);for(let a=0;a<i;a++)e[r+a]=n[this.k+a]??0;this.k+=i,s=i}else{let{phases:i,taps:a,half:o,coefs:c,step:u,len:p}=this,l=u===Math.floor(u),h=this.k,m=this.f;while(s<t){let x=m,w=h;if(!l){if(x=Math.floor(m+0.5),x>=i)x-=i,w++}if(w+o>=p)break;let I=w-o+1,M=x*a,R=0;for(let A=0;A<a;A++)R+=n[I+A]*c[M+A];if(e[r+s++]=R,m+=u,m>=i){let A=Math.floor(m/i);h+=A,m-=A*i}}this.k=h,this.f=m}return this.compact(),s}compact(){let e=this.identity?this.k:this.k-this.half+1;if(e<4096)return;this.buf.copyWithin(0,e,this.len),this.len-=e,this.k-=e}}var W=256,Z=64,f={DCH:1,CELV:2,LVBCI:4,BCIS:8,FIFOE:16},g={RPBM:1,RR:2,LVBIE:4,FEIE:8,IOCE:16},S={CODEC_READY:256,PIINT:32,POINT:64,MINT:128},j=[0,16,32],Y=0,X=1,q=2,he=3,de=W/2,pe=1;function K(){return{bdbar:0,civ:0,lvi:0,sr:f.DCH,picb:0,piv:0,cr:0,addr:0,len:0,pos:0,loaded:!1,haltedAtLast:!1,ioc:!1,bup:!1}}function ue(){return{dmaFrames:0,peakRaw:0,buffersDone:0,lastValidHalts:0,iocs:0,starvedFrames:0,irqRaised:0,bdlReads:0,capFrames:0,capSilentFrames:0,capDropped:0,capBuffers:0}}function Q(e){if(typeof e!=="object"||e===null)return!1;let t=e;if(t.version!==pe)return!1;if(!Array.isArray(t.nam)||t.nam.length!==de||!t.nam.every((r)=>Number.isInteger(r)&&r>=0&&r<=65535))return!1;if(!Number.isInteger(t.globCnt))return!1;if(!Array.isArray(t.ch)||t.ch.length!==he)return!1;return t.ch.every((r)=>typeof r==="object"&&r!==null&&Number.isInteger(r.bdbar)&&Number.isInteger(r.civ)&&Number.isInteger(r.lvi)&&Number.isInteger(r.sr)&&Number.isInteger(r.cr))}var T=48000;function L(){return{queue:new Float32Array(T),len:0,resampler:null,micRate:0,adcRate:0,scratch:new Float32Array(0),pcm:new Int16Array(0)}}class J{readMem;raiseIrq;lowerIrq;writeMem;trace;nam=new Uint16Array(de);ch=[];globCnt=0;irqAsserted=!1;play=null;playScratchL=new Float32Array(0);playScratchR=new Float32Array(0);capture=L();counters=ue();onCodecWrite=null;constructor({readMem:e,writeMem:t,raiseIrq:r=()=>{},lowerIrq:n=()=>{},trace:s=null}){this.readMem=e,this.writeMem=t??null,this.raiseIrq=r,this.lowerIrq=n,this.trace=s,this.reset()}reset(){this.resetCodec(),this.ch=Array.from({length:he},K),this.globCnt=0,this.irqAsserted=!1,this.play=null,this.capture=L(),this.counters=ue()}word(e){return this.nam[e]??0}channel(e){let t=this.ch[e];if(!t)throw RangeError("no channel "+e);return t}resetCodec(){let e=this.nam;e.fill(0),e[0]=320,e[1]=32768,e[2]=32768,e[3]=32768,e[5]=0,e[6]=32776,e[7]=32776,e[8]=34824,e[9]=34824,e[10]=34824,e[11]=34824,e[12]=34824,e[13]=0,e[14]=32768,e[15]=32768,e[16]=0,e[20]=1,e[21]=0,e[22]=48000,e[25]=48000,e[62]=33668,e[63]=30208}get vra(){return(this.word(21)&1)===1}get rate(){return this.vra?this.word(22)||48000:48000}gains(){let e=(n,s,i)=>{let a=this.word(n>>1);if(a&32768)return 0;let o=a>>s&31;return Math.pow(10,-(o-i)*1.5/20)},t=(n)=>e(2,n,0),r=(n)=>e(24,n,8);return{l:Math.min(1,t(8)*r(8)),r:Math.min(1,t(0)*r(0))}}namRead(e,t){if(t===1){let n=this.namRead(e&-2,2);return(e&1?n>>8:n)&255}if(t===4)return(this.namRead(e,2)|this.namRead(e+2,2)<<16)>>>0;e&=254;let r=this.word(e>>1);if(e===38)r=15|r&65280;if(e===42)r=r&1|128;return this.log("nam-r",e,r),r}namWrite(e,t,r){if(t===1){let n=this.word((e&-2)>>1);this.namWrite(e&-2,2,e&1?n&255|(r&255)<<8:n&65280|r&255);return}if(t===4){this.namWrite(e,2,r&65535),this.namWrite(e+2,2,r>>>16);return}switch(e&=254,r&=65535,this.log("nam-w",e,r),e){case 0:this.resetCodec();break;case 40:case 124:case 126:return;case 42:this.nam[e>>1]=r&1;return;case 44:case 50:if(this.vra)this.nam[e>>1]=H(r);return;default:this.nam[e>>1]=r}if(e<32)this.onCodecWrite?.(e,this.word(e>>1))}nabmRead(e,t){let r=0;for(let n=0;n<t;n++)r|=this.nabmRead8(e+n)<<8*n;return this.log("nabm-r"+t,e,r>>>0),r>>>0}nabmWrite(e,t,r){this.log("nabm-w"+t,e,r>>>0);for(let n=0;n<t;n++)this.nabmWrite8(e+n,r>>>8*n&255);this.updateIrq()}channelAt(e){return j.findIndex((t)=>e>=t&&e<t+12)}nabmRead8(e){if(e>=44&&e<48)return this.globCnt>>>8*(e-44)&255;if(e>=48&&e<52)return this.globSta()>>>8*(e-48)&255;if(e===52)return 0;let t=this.channelAt(e);if(t<0)return 0;let r=this.channel(t),n=e-(j[t]??0);switch(n){case 0:case 1:case 2:case 3:return r.bdbar>>>8*n&255;case 4:return r.civ;case 5:return r.lvi;case 6:return r.sr&255;case 7:return 0;case 8:return r.picb&255;case 9:return r.picb>>8&255;case 10:return r.piv;case 11:return r.cr}return 0}nabmWrite8(e,t){if(e>=44&&e<48){let i=8*(e-44);this.globCnt=(this.globCnt&~(255<<i)|t<<i)>>>0;return}if(e>=48&&e<52)return;let r=this.channelAt(e);if(r<0)return;let n=this.channel(r),s=e-(j[r]??0);switch(s){case 0:case 1:case 2:case 3:{let i=8*s;n.bdbar=((n.bdbar&~(255<<i)|t<<i)&-8)>>>0;break}case 5:if(n.lvi=t&31,n.haltedAtLast&&n.cr&g.RPBM)n.haltedAtLast=!1,n.sr&=~(f.DCH|f.CELV),n.civ=n.civ+1&31,n.loaded=!1;break;case 6:n.sr&=~(t&(f.LVBCI|f.BCIS|f.FIFOE));break;case 11:this.writeCr(r,t);break}}writeCr(e,t){let r=this.channel(e);if(t&g.RR){if(this.ch[e]=K(),e===X)this.play=null;else this.capture=L();return}let n=(r.cr&g.RPBM)!==0;if(r.cr=t&(g.RPBM|g.LVBIE|g.FEIE|g.IOCE),r.cr&g.RPBM&&!n)r.sr&=~f.DCH,r.haltedAtLast=!1,r.loaded=!1;else if(!(r.cr&g.RPBM)&&n)r.sr|=f.DCH}globSta(){let e=S.CODEC_READY,t=(r)=>Boolean(r.sr&f.BCIS&&r.cr&g.IOCE)||Boolean(r.sr&f.LVBCI&&r.cr&g.LVBIE)||Boolean(r.sr&f.FIFOE&&r.cr&g.FEIE);if(t(this.channel(0)))e|=S.PIINT;if(t(this.channel(1)))e|=S.POINT;if(t(this.channel(2)))e|=S.MINT;return e}irqPending(){return(this.globSta()&(S.PIINT|S.POINT|S.MINT))!==0}updateIrq(){let e=this.irqPending();if(e&&!this.irqAsserted)this.irqAsserted=!0,this.counters.irqRaised++,this.raiseIrq();else if(!e&&this.irqAsserted)this.irqAsserted=!1,this.lowerIrq()}get interruptAsserted(){return this.irqAsserted}releaseIrq(){if(this.irqAsserted)this.irqAsserted=!1,this.lowerIrq()}get running(){return this.channelRunning(X)}channelRunning(e){let t=this.channel(e);return(t.cr&g.RPBM)!==0&&(t.sr&f.DCH)===0}get recording(){return this.channelRunning(Y)||this.channelRunning(q)}get adcRate(){return this.vra?H(this.word(25)||48000):48000}recordGains(){let e=this.word(14);if(e&32768)return{l:0,r:0};let t=(r)=>Math.pow(10,r*1.5/20);return{l:t(e>>8&15),r:t(e&15)}}recordSources(){let e=this.word(13);return{l:(e>>8&7)===0,r:(e&7)===0}}loadBuffer(e){let t=this.readMem(e.bdbar+e.civ*8>>>0,8),r=new DataView(t.buffer,t.byteOffset,8);this.counters.bdlReads++,e.addr=(r.getUint32(0,!0)&-2)>>>0;let n=r.getUint32(4,!0);if(e.len=n&65535,e.ioc=(n&2147483648)!==0,e.bup=(n&1073741824)!==0,e.pos=0,e.picb=e.len,e.piv=e.civ+1&31,e.loaded=!0,e.civ===e.lvi)e.sr|=f.CELV;else e.sr&=~f.CELV}finishBuffer(e){if(this.counters.buffersDone++,e.ioc)e.sr|=f.BCIS,this.counters.iocs++;if(e.civ===e.lvi)e.sr|=f.LVBCI|f.DCH|f.CELV,this.counters.lastValidHalts++,e.haltedAtLast=!0,e.loaded=!1;else e.civ=e.civ+1&31,e.loaded=!1}readFrames(e,t,r,n=0,s=!1){let i=this.channel(X),a=0,o=64;while(a<e&&this.running&&o-- >0){if(!i.loaded){if(s&&i.sr&f.BCIS&&i.cr&g.IOCE)break;this.loadBuffer(i)}let c=Math.min(i.len-i.pos>>1,e-a);if(c>0){let u=this.readMem(i.addr+i.pos*2>>>0,c*4),p=new Int16Array(u.buffer.slice(u.byteOffset,u.byteOffset+c*4));for(let l=0;l<c;l++){let h=(p[2*l]??0)/32768,m=(p[2*l+1]??0)/32768;t[n+a+l]=h,r[n+a+l]=m;let x=Math.max(Math.abs(h),Math.abs(m));if(x>this.counters.peakRaw)this.counters.peakRaw=x}i.pos+=c*2,i.picb=i.len-i.pos,a+=c,this.counters.dmaFrames+=c}if(i.len-i.pos<2)i.picb=0,this.finishBuffer(i)}return this.updateIrq(),a}render(e,t,r,n,s=!1){let i=this.rate,a=this.play;if(!a||a.inRate!==i||a.outRate!==t)a=this.play={inRate:i,outRate:t,l:new O(i,t),r:new O(i,t),tail:!1,live:!1};let o=a.l.inputNeeded(e);if(o>0){if(this.running){if(this.playScratchL.length<o)this.playScratchL=new Float32Array(o),this.playScratchR=new Float32Array(o);let l=this.readFrames(o,this.playScratchL,this.playScratchR,0,s);if(l>0)a.l.write(this.playScratchL,0,l),a.r.write(this.playScratchR,0,l),a.tail=!0,a.live=!0}else if(a.tail)a.l.writeSilence(o),a.r.writeSilence(o),a.tail=!1}let c=a.l.read(r,e);a.r.read(n,e);let u=this.gains();for(let l=0;l<c;l++)r[l]=r[l]*u.l,n[l]=n[l]*u.r;for(let l=c;l<e;l++)r[l]=0,n[l]=0;if(this.running&&c<e&&!s)this.counters.starvedFrames+=e-c;let p=a.live?c:0;if(!this.running&&!a.tail)a.live=!1;return p}feedCapture(e,t,r){let n=this.capture;if(!this.recording){if(n.resampler)n.resampler.reset();n.len=0;return}let s=this.adcRate;if(!n.resampler||n.micRate!==r||n.adcRate!==s)n.resampler=new O(r,s),n.micRate=r,n.adcRate=s;n.resampler.write(e,0,t);let i=n.resampler.available();if(i>0){if(n.scratch.length<i)n.scratch=new Float32Array(i);let a=n.resampler.read(n.scratch,i);this.queueCapture(n.scratch,a,!1)}this.drainCapture()}feedSilence(e){if(!this.recording||e<=0)return;let t=this.capture;if(t.scratch.length<e)t.scratch=new Float32Array(e);t.scratch.fill(0,0,e),this.queueCapture(t.scratch,e,!0),this.drainCapture()}queueCapture(e,t,r){let n=this.capture,s=t,i=0;if(s>T)i=s-T,this.counters.capDropped+=i,s=T;let a=n.len+s-T;if(a>0)n.queue.copyWithin(0,a,n.len),n.len-=a,this.counters.capDropped+=a;if(n.queue.set(e.subarray(i,i+s),n.len),n.len+=s,r)this.counters.capSilentFrames+=s}drainCapture(){let e=this.capture,t=this.channelRunning(Y)?Y:this.channelRunning(q)?q:-1;if(t<0||!this.writeMem)return;let r=this.channel(t),n=t===q,s=n?1:2,i=this.recordGains(),a=this.recordSources(),o=0,c=64;while(o<e.len&&this.channelRunning(t)&&c-- >0){if(!r.loaded){if(r.sr&f.BCIS&&r.cr&g.IOCE)break;this.loadBuffer(r)}let u=Math.min(Math.floor((r.len-r.pos)/s),e.len-o);if(u>0){if(e.pcm.length<u*s)e.pcm=new Int16Array(u*s);for(let l=0;l<u;l++){let h=e.queue[o+l],m=a.l?Math.max(-1,Math.min(1,h*i.l)):0,x=a.r?Math.max(-1,Math.min(1,h*i.r)):0;if(n)e.pcm[l]=Math.round(m*32767);else e.pcm[2*l]=Math.round(m*32767),e.pcm[2*l+1]=Math.round(x*32767)}let p=new Uint8Array(e.pcm.buffer,0,u*s*2);this.writeMem(r.addr+r.pos*2>>>0,p),r.pos+=u*s,r.picb=r.len-r.pos,o+=u,this.counters.capFrames+=u}if(r.len-r.pos<s)r.picb=0,this.counters.capBuffers++,this.finishBuffer(r)}if(o>0)e.queue.copyWithin(0,o,e.len),e.len-=o;this.updateIrq()}log(e,t,r){if(this.trace)this.trace(e,t,r)}getState(){return{version:pe,nam:Array.from(this.nam),globCnt:this.globCnt,ch:this.ch.map((e)=>({...e}))}}setState(e){if(!Q(e))throw TypeError("not a saved AC'97 state");this.nam=Uint16Array.from(e.nam),this.globCnt=e.globCnt,this.ch=e.ch.map((t)=>({...K(),...t})),this.play=null,this.capture=L(),this.irqAsserted=!1,this.updateIrq()}}var je=`
class CaptureBlocks extends AudioWorkletProcessor {
	constructor() {
		super();
		this.block = new Float32Array(480);
		this.filled = 0;
	}
	process(inputs) {
		const input = inputs[0];
		if (input && input.length > 0) {
			const channels = input.length;
			const length = input[0].length;
			for (let i = 0; i < length; i++) {
				let sum = 0;
				for (let c = 0; c < channels; c++) sum += input[c][i];
				this.block[this.filled++] = sum / channels;
				if (this.filled === this.block.length) {
					const out = this.block;
					this.block = new Float32Array(480);
					this.filled = 0;
					this.port.postMessage(out, [out.buffer]);
				}
			}
		}
		return true;
	}
}
registerProcessor("ac97-capture", CaptureBlocks);
`;function Ye(e){switch(e){case"asking":return"Waiting for your browser's permission to use the microphone.";case"denied":return"The microphone is blocked. Allow it for this site in your browser's settings, then try again.";case"no-device":return"No microphone was found.";case"insecure":return"Browsers give a microphone only to pages served over HTTPS (or from this computer).";case"unavailable":return"This browser cannot give the page a microphone.";case"error":return"The microphone could not be started.";default:return""}}var z=new WeakMap;function Fe(e){let t=z.get(e);if(!t){let r=URL.createObjectURL(new Blob([`
class CaptureBlocks extends AudioWorkletProcessor {
	constructor() {
		super();
		this.block = new Float32Array(480);
		this.filled = 0;
	}
	process(inputs) {
		const input = inputs[0];
		if (input && input.length > 0) {
			const channels = input.length;
			const length = input[0].length;
			for (let i = 0; i < length; i++) {
				let sum = 0;
				for (let c = 0; c < channels; c++) sum += input[c][i];
				this.block[this.filled++] = sum / channels;
				if (this.filled === this.block.length) {
					const out = this.block;
					this.block = new Float32Array(480);
					this.filled = 0;
					this.port.postMessage(out, [out.buffer]);
				}
			}
		}
		return true;
	}
}
registerProcessor("ac97-capture", CaptureBlocks);
`],{type:"text/javascript"}));t=e.audioWorklet.addModule(r).finally(()=>URL.revokeObjectURL(r)),z.set(e,t),t.catch(()=>z.delete(e))}return t}class ee{context;options;current="off";stream=null;nodes=[];generation=0;lastBlockAt=Number.NEGATIVE_INFINITY;constructor(e,t){this.context=e;this.options=t}get state(){return this.current}set(e){if(this.current=e,this.options.onChange)this.options.onChange()}async start(){if(this.current==="on"||this.current==="asking")return this.current;if(typeof isSecureContext==="boolean"&&!isSecureContext)return this.set("insecure"),this.current;let e=typeof navigator==="object"?navigator.mediaDevices:void 0;if(!e||typeof e.getUserMedia!=="function"||!this.context.audioWorklet)return this.set("unavailable"),this.current;let t=++this.generation;this.set("asking");let r;try{r=await e.getUserMedia({audio:{echoCancellation:!1,noiseSuppression:!1,autoGainControl:!1,channelCount:1},video:!1})}catch(n){if(t!==this.generation)return this.current;let s=n instanceof DOMException||n instanceof Error?n.name:"";return this.set(s==="NotAllowedError"||s==="SecurityError"?"denied":s==="NotFoundError"||s==="OverconstrainedError"?"no-device":"error"),this.current}if(t!==this.generation){for(let n of r.getTracks())n.stop();return this.current}try{if(await Fe(this.context),t!==this.generation){for(let a of r.getTracks())a.stop();return this.current}let n=this.context.createMediaStreamSource(r),s=new AudioWorkletNode(this.context,"ac97-capture",{numberOfInputs:1,numberOfOutputs:1,outputChannelCount:[1]});s.port.onmessage=(a)=>{this.lastBlockAt=performance.now(),this.options.onBlock(a.data,this.context.sampleRate)};let i=this.context.createGain();i.gain.value=0,n.connect(s).connect(i).connect(this.context.destination),this.nodes=[n,s,i],this.stream=r;for(let a of r.getAudioTracks())a.addEventListener("ended",()=>this.stop());this.set("on")}catch{for(let n of r.getTracks())n.stop();this.set("error")}return this.current}stop(){if(this.generation++,this.stream)for(let e of this.stream.getTracks())e.stop();this.stream=null;for(let e of this.nodes)try{if(e instanceof AudioWorkletNode)e.port.onmessage=null;e.disconnect()}catch{}if(this.nodes=[],this.lastBlockAt=Number.NEGATIVE_INFINITY,this.current!=="off")this.set("off")}liveTracks(){return this.stream?this.stream.getTracks().filter((e)=>e.readyState==="live").length:0}}var Ke=`
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
`,te=new WeakMap;function Ne(e){let t=te.get(e);if(!t){let r=URL.createObjectURL(new Blob([`
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
`],{type:"text/javascript"}));t=e.audioWorklet.addModule(r).finally(()=>URL.revokeObjectURL(r)),te.set(e,t),t.catch(()=>te.delete(e))}return t}function me(e,t,r){let n=t.currentTime,s=e.gain;s.cancelScheduledValues(n),s.setTargetAtTime(r,n,0.01),s.setValueAtTime(r,n+0.1)}var Pe=120,fe=100;class be{context;node;kind="worklet";output;rate;onNeed=null;pushed=0;popped=0;clearing=!1;lastAsk=Number.NEGATIVE_INFINITY;target;wanted=1;pending=[];constructor(e,t){this.context=e;this.node=t;this.rate=e.sampleRate,this.target=Math.round(this.rate*Pe/1000),this.output=e.createGain(),t.connect(this.output).connect(e.destination),t.port.onmessage=(r)=>{let n=r.data;if(n.t==="level")this.popped=n.popped??this.popped,this.lastAsk=performance.now(),this.top();else if(n.t==="cleared")this.popped=n.popped??this.popped,this.pushed=this.popped,this.clearing=!1;else if(n.t==="stats"||n.t==="rec")this.pending.splice(0).forEach((s)=>s(n))}}top(){if(this.clearing)return;let e=this.target-(this.pushed-this.popped);if(e<256||!this.onNeed)return;let t=this.onNeed(e);if(!t)return;this.pushed+=e,this.node.port.postMessage({t:"push",l:t.l,r:t.r},[t.l.buffer,t.r.buffer])}live(){return this.context.state==="running"&&performance.now()-this.lastAsk<fe}setGain(e){this.wanted=e,this.ramp(e)}ramp(e){me(this.output,this.context,e)}gain(){return this.wanted}clear(){this.clearing=!0,this.node.port.postMessage({t:"clear"})}ask(e){return new Promise((t)=>{this.pending.push(t),this.node.port.postMessage(e)})}async stats(){return(await this.ask({t:"stats"})).s??null}record(e){this.node.port.postMessage({t:"rec",on:e})}async take(){let e=await this.ask({t:"take"});return{l:e.l??[],r:e.r??[]}}close(){this.node.port.onmessage=null;try{this.node.disconnect(),this.output.disconnect()}catch{}}}class ve{context;kind="script";output;rate;onNeed=null;lastAsk=Number.NEGATIVE_INFINITY;wanted=1;blocks=0;frames=0;node;constructor(e){this.context=e;this.rate=e.sampleRate,this.output=e.createGain(),this.node=e.createScriptProcessor(2048,0,2),this.node.onaudioprocess=(t)=>{let r=t.outputBuffer.getChannelData(0),n=t.outputBuffer.getChannelData(1);this.lastAsk=performance.now(),this.blocks++,this.frames+=r.length;let s=this.onNeed?this.onNeed(r.length):null;if(s)r.set(s.l),n.set(s.r);else r.fill(0),n.fill(0)},this.node.connect(this.output).connect(e.destination)}live(){return this.context.state==="running"&&performance.now()-this.lastAsk<fe*2}setGain(e){this.wanted=e,this.ramp(e)}ramp(e){me(this.output,this.context,e)}gain(){return this.wanted}clear(){}async stats(){return{out:this.frames,silent:0,peak:0,underrunEvents:0,underrunFrames:0,dropped:0,quanta:this.blocks}}record(){}async take(){return{l:[],r:[]}}close(){this.node.onaudioprocess=null;try{this.node.disconnect(),this.output.disconnect()}catch{}}}async function ge(e){if(e.audioWorklet&&typeof AudioWorkletNode==="function")try{await Ne(e);let t=new AudioWorkletNode(e,"ac97-ring-player",{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2]});return new be(e,t)}catch{}return new ve(e)}var Oe="0.5.473",xe=53248,ye=54272;function Ae(){let e=new Uint8Array(64),t=(r,n)=>{e[r]=n&255,e[r+1]=n>>8};return t(0,32902),t(2,9237),t(4,3),t(6,640),e[8]=2,e[10]=1,e[11]=4,t(16,xe|1),t(20,ye|1),t(44,32902),t(46,0),e[61]=1,e[62]=2,e[63]=24,Array.from(e)}function re(e,t=null){let r=e.devices.pci,n=3;while(n<32&&r.devices[n<<3])n++;if(n>=32)throw Error("no free PCI slot for the AC'97 device");let s=n<<3,i=!0,a=new J({readMem:(c,u)=>e.read_blob(c,u),writeMem:(c,u)=>e.write_blob(u,c),raiseIrq:()=>i&&r.raise_irq(s),lowerIrq:()=>r.lower_irq(s),trace:t}),o=(c,u,p,l)=>{for(let h=0;h<u;h++)e.io.register_read(c+h,a,()=>i?p(h,1):255,()=>i?p(h,2):65535,()=>i?p(h,4):4294967295),e.io.register_write(c+h,a,(m)=>i&&l(h,1,m),(m)=>i&&l(h,2,m),(m)=>i&&l(h,4,m))};return o(xe,W,(c,u)=>a.namRead(c,u),(c,u,p)=>a.namWrite(c,u,p)),o(ye,Z,(c,u)=>a.nabmRead(c,u),(c,u,p)=>a.nabmWrite(c,u,p)),r.register_device({pci_id:s,pci_space:Ae(),pci_bars:[{size:W},{size:Z}],name:"AC97"}),{model:a,pciId:s,retire(){a.releaseIrq(),i=!1}}}function ne(e,t){let r=e.v86;if(r&&r.cpu&&r.cpu.devices&&r.cpu.devices.pci)return t(r.cpu),()=>{};let n=r,s=null,i=!1,a=(o)=>{let c=o.init;s=()=>{o.init=c,s=null},o.init=(...u)=>{let p=c.apply(o,u);if(s)s();if(!i)i=!0,t(o.cpu);return p}};if(Object.defineProperty(e,"v86",{configurable:!0,enumerable:!0,get:()=>n,set(o){if(n=o,o&&!i)a(o)}}),n)a(n);return()=>{if(i=!0,s)s();Object.defineProperty(e,"v86",{configurable:!0,enumerable:!0,writable:!0,value:n})}}var Ce=(e,t)=>Math.round(e*100/t),se=(e)=>100-Ce(e&31,31),Te=(e)=>Ce(e&15,15),B=(e,t,r=8)=>({raw:e,left:t(e>>r),right:t(e),muted:e>32767});function Be(e){let t=(r)=>e.word(r>>1);return{master:B(t(2),se),pcm:B(t(24),se),record:B(t(28),Te),mic:B(t(14),se,0),recordSelect:B(t(26),(r)=>r&7),rate:e.rate,vra:e.vra}}var ie=(e)=>Number.isFinite(e)?Math.min(1,Math.max(0,e)):0;function we(e,t){let r=ie(e);return t?0:r*r}var _e=10,D=4800,Ve=500,qe=50;function nt(e,t={}){let r=ie(t.volume??0.7),n=Boolean(t.muted),s=null,i=null,a=!1,o=!1,c=!1,u="",p=0,l=performance.now(),h=null,m=()=>{},x=new Promise((d)=>m=d),w=()=>{if(!o&&t.onChange)t.onChange()},I=new Float32Array(D),M=new Float32Array(D),R=()=>{let d=performance.now();if(i&&i.live()){l=d;return}let v=d-l;l=d;let y=s?s.model:null;if(!y||!y.running)return;let k=i?i.rate:48000,N=Math.min(D,Math.round(v*k/1000));if(N<1)return;y.render(N,k,I,M,!0),p+=N},A=performance.now(),C=()=>{let d=performance.now(),v=d-A;A=d;let y=s?s.model:null;if(!y||!y.recording)return;if(b&&b.state==="on"&&d-b.lastBlockAt<Ve)return;let k=Math.min(D,Math.round(v*y.adcRate/1000));if(k>0)y.feedSilence(k)},_=setInterval(()=>{R(),C()},_e),b=null,P=()=>{if(i)i.setGain(we(r,n))},E=()=>w(),U=(d)=>{ge(d).then((v)=>{if(o){v.close();return}i=v,i.onNeed=(y)=>{let k=s?s.model:null;if(!k)return null;let N=new Float32Array(y),oe=new Float32Array(y);return k.render(y,v.rate,N,oe),{l:N,r:oe}},P(),w()},()=>{a=!0,w()})},F=new Map,V=null,Me=()=>{V=null;for(let[d,v]of F)t.onCodec?.(d,v);F.clear()},ae=typeof AudioContext==="function"?AudioContext:null;if(t.context)h=t.context;else if(t.context===void 0&&ae)try{h=new ae({sampleRate:48000,latencyHint:"interactive"}),c=!0}catch{a=!0}if(h)h.addEventListener("statechange",E),U(h);if(h)b=new ee(h,{onBlock:(d,v)=>{if(s&&!o)s.model.feedCapture(d,d.length,v)},onChange:w});let ke=ne(e,(d)=>{if(o)return;if(s=re(d,t.trace??null),t.onCodec)s.model.onCodecWrite=(v,y)=>{F.set(v,y),V??=setTimeout(Me,qe)};if(t.state)if(Q(t.state))try{s.model.setState(t.state)}catch(v){u=v instanceof Error?v.message:String(v)}else u="the saved registers are not an AC'97 state";m(),w()}),G=()=>o?"none":a?"failed":h?h.state:"none";return{getAc97State:()=>s?s.model.getState():null,setVolume(d){r=ie(d),P()},getVolume:()=>r,setMuted(d){n=d,P()},isMuted:()=>n,async resume(){if(h&&h.state!=="running"&&h.state!=="closed")try{await h.resume()}catch{}return G()},outputState:G,codec:()=>s?Be(s.model):null,stats(){let d=s?s.model:null;return{registered:s!==null,pciId:s?s.pciId:null,guestRate:d?d.rate:0,playing:d?d.running:!1,interrupt:d?d.interruptAsserted:!1,counters:d?{...d.counters}:null,drainedFrames:p,output:G(),mic:b?b.state:"off",recording:d?d.recording:!1,sink:i?i.kind:null,volume:r,muted:n,gain:we(r,n),stateError:u}},startMicrophone:async()=>b&&!o?b.start():"unavailable",stopMicrophone(){if(b)b.stop()},micState:()=>b?b.state:"off",get microphone(){return b},sinkStats:async()=>i?i.stats():null,get sink(){return i},get model(){return s?s.model:null},registered:x,detach(){if(o)return;if(o=!0,clearInterval(_),V)clearTimeout(V);if(b)b.stop();if(ke(),s)s.retire();if(i)i.clear(),i.close();if(h){if(h.removeEventListener("statechange",E),c)h.close().catch(()=>{})}i=null,m()}}}export{J as Ac97,je as CAPTURE_WORKLET,g as CR,S as GS,De as MAX_RATE,We as MIN_RATE,ee as Microphone,Z as NABM_SIZE,W as NAM_SIZE,O as Resampler,f as SR,Le as STANDARD_RATES,Oe as V86_VERSION,Ke as WORKLET_SOURCE,nt as attachAc97,H as clampRate,ge as createSink,we as gainFor,Q as isAc97State,Ye as micReason,ne as onceBeforeInit,Ae as pciSpace,re as registerOn};
