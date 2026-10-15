import ghidra.app.script.GhidraScript; import ghidra.program.model.listing.*; import ghidra.program.model.mem.*; import ghidra.program.model.address.*;
public class SweepConstants extends GhidraScript { public void run()throws Exception{for(long a:new long[]{0x005ddb9cL}){Address x=toAddr(a);println(x+" "+getDataAt(x));try{println("f32="+getFloat(x));}catch(Exception e){println(e);}}}}
