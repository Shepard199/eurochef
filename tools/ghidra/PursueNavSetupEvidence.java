import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
public class PursueNavSetupEvidence extends GhidraScript {
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionAt(a);if(f==null)f=createFunction(a,name);
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  long[] xs={0x0046DAA0L,0x0046D9E0L,0x0046DE60L,0x0046E070L};
  for(long x:xs)dc(x,"tmp_"+Long.toHexString(x),d);
  // Find direct byte writes to +9 in the PursueNav code range.
  InstructionIterator it=currentProgram.getListing().getInstructions(toAddr(0x0046D900L),true);
  int n=0; while(it.hasNext()&&n++<1200){Instruction ins=it.next(); long a=ins.getAddress().getOffset(); if(a>=0x0046E300L)break;
    String s=ins.toString().toLowerCase();
    if(s.contains("+ 0x9]")||s.contains("+0x9]")||s.contains("+ 0x26]")||s.contains("+0x26]")) println(ins.getAddress()+" "+ins);
  }
  d.dispose();
 }
}