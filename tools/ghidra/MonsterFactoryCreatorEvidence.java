import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
public class MonsterFactoryCreatorEvidence extends GhidraScript {
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionAt(a);if(f==null)f=createFunction(a,name);
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;DecompileResults r=d.decompileFunction(f,150,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  dc(0x0047E8B0L,"tmp_MonsterFactory",d);
  dc(0x0047ECB0L,"tmp_MonsterFactoryBind",d);
  dc(0x0047E6F0L,"tmp_MonsterFactoryFinalize",d);
  d.dispose();
 }
}