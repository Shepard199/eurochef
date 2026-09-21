import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class CurrentAttackerBonusSetterEvidence extends GhidraScript {
 @Override public void run()throws Exception{
  Function f=getFunctionContaining(toAddr(0x00456093L));
  println("FUNCTION="+(f==null?"<missing>":f.getName()+"@"+f.getEntryPoint()));
  if(f==null)return;
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  DecompileResults r=d.decompileFunction(f,150,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
  d.dispose();
 }
}