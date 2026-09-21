import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
public class BehaviorNodeBaseFlagsEvidence extends GhidraScript {
 private void dc(long raw,String name,DecompInterface d)throws Exception{
  Address a=toAddr(raw);disassemble(a);Function f=getFunctionAt(a);if(f==null)f=createFunction(a,name);
  println(String.format("\n=== 0x%08X %s ===",raw,f==null?"<missing>":f.getName()));
  if(f==null)return;DecompileResults r=d.decompileFunction(f,90,monitor);
  if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
 }
 @Override public void run()throws Exception{
  DecompInterface d=new DecompInterface();d.openProgram(currentProgram);
  dc(0x00456E60L,"tmp_BehaviorBaseCtor",d);
  dc(0x00456EC0L,"tmp_BehaviorBaseReset",d);
  dc(0x00456F00L,"tmp_BehaviorEnter",d);
  // Print all instructions in behavior-base region that access byte +9 off ECX/ESI/EDI.
  InstructionIterator it=currentProgram.getListing().getInstructions(toAddr(0x00456E00L),true);
  int n=0;while(it.hasNext()&&n++<500){Instruction ins=it.next();long a=ins.getAddress().getOffset();if(a>=0x00457400L)break;
    String s=ins.toString().toLowerCase();
    if(s.contains("+ 0x9]")||s.contains("+0x9]"))println(ins.getAddress()+" "+ins);
  }
  d.dispose();
 }
}