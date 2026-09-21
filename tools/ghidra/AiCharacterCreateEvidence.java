import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class AiCharacterCreateEvidence extends GhidraScript {
  private void dec(long raw,DecompInterface d)throws Exception{
    Function f=getFunctionAt(toAddr(raw));if(f==null)f=getFunctionContaining(toAddr(raw));
    println(String.format("\n=== %08X %s ===",raw,f==null?"<missing>":f.getName()));
    if(f!=null){DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}
  }
  @Override public void run()throws Exception{
    DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
    dec(0x0047E740L,d); dec(0x0047E4F0L,d);
    d.dispose();
  }
}