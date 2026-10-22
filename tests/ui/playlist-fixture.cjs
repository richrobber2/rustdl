const {JSDOM}=require('jsdom');const fs=require('node:fs');const path=require('node:path');const script=require('./read-script.cjs');
module.exports=(count=500,patch={})=>{
 const entries=Array.isArray(count)?count:Array.from({length:count},(_,index)=>({video_id:'id'+String(index).padStart(9,'0'),title:'Synthetic video '+index,author:index<250?'Artist A':'Artist B',...patch}));
 const html=fs.readFileSync(path.join(__dirname,'../../assets/html/playlist.html'),'utf8').replace(/<style>[\s\S]*?<\/style>/,'').replace('{token}','synthetic-token').replace('{entries}',JSON.stringify(entries).replace(/</g,'\\u003c'));
 const dom=new JSDOM(html,{url:'http://localhost/',runScripts:'outside-only',pretendToBeVisual:true});dom.window.scrollTo=()=>{};dom.window.eval(script('playlist.js'));return dom;
};
