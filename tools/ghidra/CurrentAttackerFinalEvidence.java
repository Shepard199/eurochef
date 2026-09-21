import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.address.Address;
import ghidra.program.model.symbol.Reference;
public class CurrentAttackerFinalEvidence extends GhidraScript {
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionContaining(a);if(f==null){f=getFunctionAt(a);if(f==null)f=createFunction(a,name);}
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;DecompileResults r=d.decompileFunction(f,120,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  println(String.format("DAT_005DD384 bits=0x%08X float=%.9f",Integer.toUnsignedLong(getInt(toAddr(0x005DD384L))),Float.intBitsToFloat(getInt(toAddr(0x005DD384L)))));
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  dc(0x0041CDC0L,"tmp_NpcPriority14C",d);
  dc(0x00455560L,"tmp_AttackerEnvironmentBlock",d);
  dc(0x00457C40L,"tmp_CommonHitEnter",d);
  long g=0x007B2A14L;
  println("\n=== refs 0x007B2A14 ===");
  for(Reference ref:getReferencesTo(toAddr(g))){
   Function f=getFunctionContaining(ref.getFromAddress());
   println(ref.getFromAddress()+" "+ref.getReferenceType()+" "+(f==null?"NOFUNC":f.getName()+"@"+f.getEntryPoint()));
   if(f!=null)dc(f.getEntryPoint().getOffset(),"tmp_bonus_ref",d);
  }
  d.dispose();
 }
}