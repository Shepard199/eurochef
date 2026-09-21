import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class PiranhaMovementHelpersEvidence extends GhidraScript {
  private void dec(long raw,DecompInterface d)throws Exception{
    disassemble(toAddr(raw)); Function f=getFunctionAt(toAddr(raw));
    if(f==null){try{f=createFunction(toAddr(raw),"evidence_"+Long.toHexString(raw));}catch(Exception ignored){}}
    if(f==null)f=getFunctionContaining(toAddr(raw));
    println(String.format("\n=== %08X %s ===",raw,f==null?"<missing>":f.getName()));
    if(f!=null){DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
  }
  private void fl(long raw)throws Exception{
    int bits=getInt(toAddr(raw));
    println(String.format("FLOAT %08X bits=%08X value=%s",raw,bits,Float.toString(Float.intBitsToFloat(bits))));
  }
  @Override public void run()throws Exception{
    DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
    dec(0x00454CA0L,d); dec(0x0041EC80L,d); dec(0x00467B50L,d); dec(0x0044CD10L,d);
    fl(0x005DD380L); fl(0x005DD384L); fl(0x005DD388L); fl(0x005DE534L); fl(0x005DD72CL); fl(0x00620034L);
    d.dispose();
  }
}