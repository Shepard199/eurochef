import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class CurrentAttackerManagerCtorOwnerEvidence extends GhidraScript {
 @Override public void run()throws Exception{
  Address a=toAddr(0x00455F00L); disassemble(a);
  Function f=getFunctionAt(a); if(f==null)f=createFunction(a,"tmp_AttackerManagerOwnerCtor");
  println("FUNCTION="+(f==null?"<missing>":f.getName()+"@"+f.getEntryPoint()));
  if(f==null)return;
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  DecompileResults r=d.decompileFunction(f,150,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
  d.dispose();
 }
}