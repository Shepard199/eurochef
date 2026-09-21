import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class PiranhaAttackBaseEvidence extends GhidraScript {
  private void dec(long raw,DecompInterface d)throws Exception{
    Function f=getFunctionAt(toAddr(raw)); if(f==null)f=getFunctionContaining(toAddr(raw));
    println(String.format("\n=== %08X %s ===",raw,f==null?"<missing>":f.getName()));
    if(f!=null){DecompileResults r=d.decompileFunction(f,60,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
  }
  @Override public void run()throws Exception{
    DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
    dec(0x00456E60L,d);
    int bits=getInt(toAddr(0x005E1FA8L));
    println(String.format("DAT_005E1FA8 bits=%08X value=%s",bits,Float.toString(Float.intBitsToFloat(bits))));
    d.dispose();
  }
}