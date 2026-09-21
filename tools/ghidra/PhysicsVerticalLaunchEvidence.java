import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class PhysicsVerticalLaunchEvidence extends GhidraScript {
 @Override public void run() throws Exception {
  long[] addrs={0x0041A370L,0x00419400L};
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  for(long a:addrs){Function f=getFunctionContaining(toAddr(a));println(String.format("\n=== 0x%08X %s ===",a,f==null?"<missing>":f.getName()));if(f!=null){DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());}}
  d.dispose();
 }
}