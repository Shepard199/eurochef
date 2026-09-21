import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class DistanceTriggerOverridesEvidence extends GhidraScript {
  @Override public void run() throws Exception {
    long[] addrs={0x0048D050L,0x0048D070L,0x0048D0A0L,0x0048D140L,0x0048D1C0L,0x0048D2B0L,0x0048D2D0L,0x0048D2F0L,0x0048D490L,0x0048FA50L,0x0048FA60L,0x0048FA70L,0x0048FAF0L};
    DecompInterface d=new DecompInterface(); d.openProgram(currentProgram);
    for(long a:addrs){
      Function f=getFunctionContaining(toAddr(a));
      println(String.format("\n=== 0x%08X %s ===",a,f==null?"<missing>":f.getName()));
      if(f==null) continue;
      DecompileResults r=d.decompileFunction(f,60,monitor);
      if(r.decompileCompleted()&&r.getDecompiledFunction()!=null) println(r.getDecompiledFunction().getC());
    }
    d.dispose();
  }
}