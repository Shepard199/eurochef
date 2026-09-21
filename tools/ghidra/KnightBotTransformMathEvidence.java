import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class KnightBotTransformMathEvidence extends GhidraScript {
 private void f(long a,String n)throws Exception{int bits=getInt(toAddr(a));println(String.format("%s %08X %08X %.9g",n,a,bits,Float.intBitsToFloat(bits)));}
 private void dc(long a,DecompInterface d)throws Exception{Function fn=getFunctionContaining(toAddr(a));println(String.format("\n===%08X %s===",a,fn==null?"<missing>":fn.getName()));if(fn!=null){DecompileResults r=d.decompileFunction(fn,90,monitor);if(r.decompileCompleted())println(r.getDecompiledFunction().getC());}}
 @Override public void run() throws Exception {f(0x005DD734L,"DAT_005dd734");f(0x005DD380L,"DAT_005dd380");DecompInterface d=new DecompInterface();d.openProgram(currentProgram);dc(0x005094A6L,d);dc(0x004543A0L,d);d.dispose();}
}