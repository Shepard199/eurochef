import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class GenericAttackSetupEvidence extends GhidraScript {
  private long ptr(long a)throws Exception{return Integer.toUnsignedLong(getInt(toAddr(a)));}
  private void dec(long raw,DecompInterface d)throws Exception{
    disassemble(toAddr(raw)); Function f=getFunctionAt(toAddr(raw));
    if(f==null){try{f=createFunction(toAddr(raw),"evidence_"+Long.toHexString(raw));}catch(Exception ignored){}}
    if(f==null)f=getFunctionContaining(toAddr(raw));
    println(String.format("\n=== %08X %s ===",raw,f==null?"<missing>":f.getName()));
    if(f!=null){DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
  }
  @Override public void run()throws Exception{
    DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
    long v=0x005E1FBCL;
    for(int off=0;off<=0x40;off+=4)println(String.format("+%02X -> %08X",off,ptr(v+off)));
    dec(ptr(v+0x38),d); dec(ptr(v+0x18),d); dec(ptr(v+0x10),d);
    d.dispose();
  }
}