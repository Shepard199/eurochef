import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;

public class RobotsInputActionTest2170Evidence extends GhidraScript {
 void one(long x)throws Exception{
  Address a=toAddr(x); disassemble(a);
  Function f=getFunctionAt(a);
  if(f==null){try{f=createFunction(a,null);}catch(Exception e){println("CREATE_FAIL "+Long.toHexString(x)+" "+e.getMessage());}}
  println(String.format("\n=== 0x%08X ===",x));
  InstructionIterator it=currentProgram.getListing().getInstructions(a,true);
  int n=0; while(it.hasNext()&&n++<80){Instruction ins=it.next();println(ins.getAddress()+"  "+ins);if(ins.getFlowType().isTerminal())break;}
  if(f!=null){DecompInterface d=new DecompInterface();d.openProgram(currentProgram);DecompileResults r=d.decompileFunction(f,90,monitor);if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());d.dispose();}
 }
 public void run()throws Exception{long[] a={0x004d2110L,0x004d2130L,0x004d2150L,0x004d2170L,0x004d2190L,0x004d21b0L,0x004d21e0L};for(long x:a)one(x);}
}